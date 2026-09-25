//! Alibaba DashScope Qwen realtime ASR client.
//!
//! Uses the OpenAI Realtime-style WebSocket protocol exposed by DashScope for
//! `qwen3-asr-flash-realtime`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex as ParkingMutex;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex, Notify};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use url::Url;
use uuid::Uuid;

use super::{AudioConsumer, RawTranscript};

pub const PROVIDER_ID: &str = "qwen3-asr-flash-realtime";
pub const DEFAULT_ENDPOINT: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/realtime";
pub const DEFAULT_MODEL: &str = "qwen3-asr-flash-realtime";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QwenRealtimePreset {
    pub endpoint_cn: &'static str,
    pub model: &'static str,
}

pub fn qwen_realtime_preset() -> QwenRealtimePreset {
    QwenRealtimePreset {
        endpoint_cn: DEFAULT_ENDPOINT,
        model: DEFAULT_MODEL,
    }
}

/// 100 ms of 16 kHz / 16-bit / mono PCM.
pub const TARGET_AUDIO_CHUNK_BYTES: usize = 3_200;
const BYTES_PER_MS: u64 = 32;
const FINAL_RESULT_TIMEOUT: Duration = Duration::from_secs(12);
const CONNECTION_RETRY_DELAYS: [Duration; 2] =
    [Duration::from_millis(250), Duration::from_millis(750)];

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = futures_util::stream::SplitSink<WsStream, Message>;
type SharedWriter = Arc<AsyncMutex<Option<WsSink>>>;

#[derive(Clone, Debug)]
pub struct QwenRealtimeCredentials {
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
}

impl QwenRealtimeCredentials {
    pub fn normalized_endpoint(&self) -> String {
        if self.endpoint.trim().is_empty() {
            DEFAULT_ENDPOINT.to_string()
        } else {
            self.endpoint.trim().to_string()
        }
    }

    pub fn normalized_model(&self) -> String {
        let model = self.model.trim();
        if model.is_empty() {
            DEFAULT_MODEL.to_string()
        } else {
            model.to_string()
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QwenRealtimeASRError {
    #[error("credentials missing")]
    CredentialsMissing,
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("task failed: {0}")]
    TaskFailed(String),
    #[error("no final result")]
    NoFinalResult,
    #[error("final result timed out")]
    FinalResultTimeout,
}

enum SendItem {
    Audio(Vec<u8>),
    Finish(oneshot::Sender<Result<(), QwenRealtimeASRError>>),
}

#[derive(Default)]
struct SyncState {
    pending_audio: Vec<u8>,
    audio_scratch: Vec<u8>,
    bytes_received: u64,
    session_started: bool,
    session_finished: bool,
    closing: bool,
    runtime: Option<Handle>,
    start: Option<Instant>,
    final_tx: Option<oneshot::Sender<Result<RawTranscript, QwenRealtimeASRError>>>,
    send_tx: Option<mpsc::UnboundedSender<SendItem>>,
    final_segments: Vec<String>,
    last_partial_text: String,
}

pub struct QwenRealtimeASR {
    credentials: QwenRealtimeCredentials,
    state: ParkingMutex<SyncState>,
    writer: SharedWriter,
    final_rx: ParkingMutex<Option<oneshot::Receiver<Result<RawTranscript, QwenRealtimeASRError>>>>,
    session_started: Arc<Notify>,
    send_task: ParkingMutex<Option<tokio::task::JoinHandle<()>>>,
    read_task: ParkingMutex<Option<tokio::task::JoinHandle<()>>>,
}

impl QwenRealtimeASR {
    pub fn new(credentials: QwenRealtimeCredentials) -> Self {
        Self {
            credentials,
            state: ParkingMutex::new(SyncState::default()),
            writer: Arc::new(AsyncMutex::new(None)),
            final_rx: ParkingMutex::new(None),
            session_started: Arc::new(Notify::new()),
            send_task: ParkingMutex::new(None),
            read_task: ParkingMutex::new(None),
        }
    }

    pub async fn open_session(self: &Arc<Self>) -> Result<(), QwenRealtimeASRError> {
        if self.credentials.api_key.trim().is_empty() {
            return Err(QwenRealtimeASRError::CredentialsMissing);
        }

        let endpoint = realtime_endpoint_with_model(
            &self.credentials.normalized_endpoint(),
            &self.credentials.normalized_model(),
        )?;
        let ws = connect_with_retry(&endpoint, self.credentials.api_key.trim()).await?;
        let (write, read) = ws.split();
        *self.writer.lock().await = Some(write);

        let (final_tx, final_rx) = oneshot::channel();
        let (send_tx, mut send_rx) = mpsc::unbounded_channel::<SendItem>();
        {
            let mut st = self.state.lock();
            *st = SyncState::default();
            st.runtime = Some(Handle::current());
            st.start = Some(Instant::now());
            st.final_tx = Some(final_tx);
            st.send_tx = Some(send_tx);
        }
        *self.final_rx.lock() = Some(final_rx);

        let writer_for_worker = Arc::clone(&self.writer);
        let weak_for_worker = Arc::downgrade(self);
        *self.send_task.lock() = Some(tokio::spawn(async move {
            while let Some(item) = send_rx.recv().await {
                match item {
                    SendItem::Audio(chunk) => {
                        if let Err(e) =
                            send_text(&writer_for_worker, append_audio_message(&chunk)).await
                        {
                            log::error!("[qwen-realtime-asr] audio frame send failed: {e}");
                            if let Some(this) = weak_for_worker.upgrade() {
                                this.finish_error(e);
                            }
                            break;
                        }
                    }
                    SendItem::Finish(done) => {
                        let result = send_text(&writer_for_worker, session_finish_message())
                            .await
                            .map_err(|e| QwenRealtimeASRError::SendFailed(e.to_string()));
                        let _ = done.send(result);
                    }
                }
            }
        }));

        send_text(&self.writer, session_update_message()).await?;

        let weak_self = Arc::downgrade(self);
        *self.read_task.lock() = Some(tokio::spawn(async move {
            let mut read = read;
            while let Some(msg) = read.next().await {
                let Some(this) = weak_self.upgrade() else {
                    break;
                };
                match msg {
                    Ok(Message::Text(text)) => {
                        if !this.handle_text_message(&text) {
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        this.finish_with_partial_or_error(QwenRealtimeASRError::NoFinalResult);
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("[qwen-realtime-asr] receive loop error: {e}");
                        this.finish_with_partial_or_error(QwenRealtimeASRError::ConnectionFailed(
                            e.to_string(),
                        ));
                        break;
                    }
                }
            }
        }));

        Ok(())
    }

    pub async fn send_last_frame(&self) -> Result<(), QwenRealtimeASRError> {
        let started = self.session_started.notified();
        tokio::pin!(started);
        started.as_mut().enable();
        if !self.state.lock().session_started {
            tokio::time::timeout(FINAL_RESULT_TIMEOUT, &mut started)
                .await
                .map_err(|_| QwenRealtimeASRError::FinalResultTimeout)?;
        }
        let (done_tx, done_rx) = oneshot::channel();
        {
            let mut st = self.state.lock();
            if st.closing || st.session_finished || !st.session_started {
                return Err(QwenRealtimeASRError::SendFailed(
                    "session closed".to_string(),
                ));
            }
            st.closing = true;
            let pending = std::mem::take(&mut st.pending_audio);
            st.audio_scratch.extend_from_slice(&pending);
            enqueue_audio_chunks(&mut st, true)?;
            st.send_tx
                .as_ref()
                .ok_or_else(|| QwenRealtimeASRError::SendFailed("send worker missing".to_string()))?
                .send(SendItem::Finish(done_tx))
                .map_err(|_| QwenRealtimeASRError::SendFailed("send worker closed".to_string()))?;
        }
        tokio::time::timeout(FINAL_RESULT_TIMEOUT, done_rx)
            .await
            .map_err(|_| QwenRealtimeASRError::FinalResultTimeout)?
            .map_err(|_| QwenRealtimeASRError::SendFailed("finish ack dropped".to_string()))?
    }

    pub async fn await_final_result(&self) -> Result<RawTranscript, QwenRealtimeASRError> {
        let rx = self.final_rx.lock().take();
        let Some(rx) = rx else {
            return Err(QwenRealtimeASRError::NoFinalResult);
        };
        tokio::time::timeout(FINAL_RESULT_TIMEOUT, rx)
            .await
            .map_err(|_| QwenRealtimeASRError::FinalResultTimeout)?
            .map_err(|_| QwenRealtimeASRError::NoFinalResult)?
    }

    pub fn cancel(&self) {
        let runtime = {
            let mut st = self.state.lock();
            st.closing = true;
            st.session_finished = true;
            st.pending_audio.clear();
            st.audio_scratch.clear();
            st.send_tx.take();
            st.final_tx.take();
            st.runtime.clone()
        };
        self.session_started.notify_waiters();
        if let Some(task) = self.send_task.lock().take() {
            task.abort();
        }
        if let Some(task) = self.read_task.lock().take() {
            task.abort();
        }
        let writer = Arc::clone(&self.writer);
        if let Some(handle) = runtime {
            handle.spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(2), close_writer(&writer)).await;
            });
        }
    }

    fn handle_text_message(&self, text: &str) -> bool {
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            log::warn!("[qwen-realtime-asr] non-json text message: {text}");
            return true;
        };
        let event_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match event_type {
            "session.updated" => self.mark_session_started(),
            "conversation.item.input_audio_transcription.text" => {
                if let Some(text) = extract_transcript_text(&value) {
                    self.state.lock().last_partial_text = text.to_string();
                }
            }
            "conversation.item.input_audio_transcription.completed" => {
                if let Some(text) = extract_transcript_text(&value) {
                    self.state.lock().final_segments.push(text.to_string());
                }
            }
            "session.finished" => {
                let transcript = extract_transcript_text(&value).map(str::to_string);
                self.finish_success(transcript);
                return false;
            }
            "error" => {
                let message = value
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .or_else(|| value.get("message").and_then(Value::as_str))
                    .unwrap_or("provider returned error");
                self.finish_error(QwenRealtimeASRError::TaskFailed(message.to_string()));
                return false;
            }
            _ => {}
        }
        true
    }

    fn mark_session_started(&self) {
        {
            let mut st = self.state.lock();
            if st.session_started || st.closing {
                return;
            }
            st.session_started = true;
            let pending = std::mem::take(&mut st.pending_audio);
            st.audio_scratch.extend_from_slice(&pending);
            if let Err(error) = enqueue_audio_chunks(&mut st, false) {
                log::error!("[qwen-realtime-asr] pending audio enqueue failed: {error}");
            }
        }
        self.session_started.notify_waiters();
    }

    fn finish_success(&self, transcript: Option<String>) {
        let (final_tx, raw) = {
            let mut st = self.state.lock();
            if st.session_finished {
                return;
            }
            st.session_finished = true;
            let joined = st.final_segments.join("");
            let text = if !joined.trim().is_empty() {
                joined
            } else if let Some(transcript) = transcript {
                transcript
            } else {
                st.last_partial_text.clone()
            };
            let duration_ms = st.bytes_received / BYTES_PER_MS;
            let raw = RawTranscript { text, duration_ms };
            (st.final_tx.take(), raw)
        };
        if let Some(tx) = final_tx {
            let _ = tx.send(Ok(raw));
        }
    }

    fn finish_with_partial_or_error(&self, error: QwenRealtimeASRError) {
        let partial = {
            let st = self.state.lock();
            let joined = st.final_segments.join("");
            if !joined.trim().is_empty() {
                Some(joined)
            } else if !st.last_partial_text.trim().is_empty() {
                Some(st.last_partial_text.clone())
            } else {
                None
            }
        };
        if partial.is_some() {
            self.finish_success(partial);
        } else {
            self.finish_error(error);
        }
    }

    fn finish_error(&self, error: QwenRealtimeASRError) {
        let final_tx = {
            let mut st = self.state.lock();
            if st.session_finished {
                return;
            }
            st.session_finished = true;
            st.final_tx.take()
        };
        if let Some(tx) = final_tx {
            let _ = tx.send(Err(error));
        }
    }
}

impl AudioConsumer for QwenRealtimeASR {
    fn consume_pcm_chunk(&self, pcm: &[u8]) {
        if pcm.is_empty() {
            return;
        }
        let mut st = self.state.lock();
        if st.closing || st.session_finished {
            return;
        }
        st.bytes_received = st.bytes_received.saturating_add(pcm.len() as u64);
        if !st.session_started {
            st.pending_audio.extend_from_slice(pcm);
            return;
        }
        st.audio_scratch.extend_from_slice(pcm);
        if let Err(error) = enqueue_audio_chunks(&mut st, false) {
            log::error!("[qwen-realtime-asr] audio enqueue failed: {error}");
        }
    }
}

fn enqueue_audio_chunks(st: &mut SyncState, flush_tail: bool) -> Result<(), QwenRealtimeASRError> {
    let tx = st
        .send_tx
        .as_ref()
        .ok_or_else(|| QwenRealtimeASRError::SendFailed("send worker missing".to_string()))?
        .clone();
    if flush_tail && st.audio_scratch.len() % 2 != 0 {
        return Err(QwenRealtimeASRError::SendFailed(
            "unaligned PCM tail".to_string(),
        ));
    }
    while st.audio_scratch.len() >= TARGET_AUDIO_CHUNK_BYTES {
        let chunk = st.audio_scratch.drain(..TARGET_AUDIO_CHUNK_BYTES).collect();
        tx.send(SendItem::Audio(chunk))
            .map_err(|_| QwenRealtimeASRError::SendFailed("send worker closed".to_string()))?;
    }
    if flush_tail && !st.audio_scratch.is_empty() {
        let tail = std::mem::take(&mut st.audio_scratch);
        tx.send(SendItem::Audio(tail))
            .map_err(|_| QwenRealtimeASRError::SendFailed("send worker closed".to_string()))?;
    }
    Ok(())
}

fn realtime_endpoint_with_model(
    endpoint: &str,
    model: &str,
) -> Result<String, QwenRealtimeASRError> {
    let mut url =
        Url::parse(endpoint).map_err(|e| QwenRealtimeASRError::ConnectionFailed(e.to_string()))?;
    url.query_pairs_mut().append_pair("model", model);
    Ok(url.to_string())
}

async fn connect_with_retry(
    endpoint: &str,
    api_key: &str,
) -> Result<WsStream, QwenRealtimeASRError> {
    for (retry_index, delay) in CONNECTION_RETRY_DELAYS.iter().enumerate() {
        match connect_once(endpoint, api_key).await {
            Ok(ws) => return Ok(ws),
            Err(error) if is_retryable_connection_error(&error) => {
                log::warn!(
                    "[qwen-realtime-asr] transient connection failure; retrying ({}/{}): {}",
                    retry_index + 1,
                    CONNECTION_RETRY_DELAYS.len(),
                    error
                );
                tokio::time::sleep(*delay).await;
            }
            Err(error) => return Err(QwenRealtimeASRError::ConnectionFailed(error)),
        }
    }

    connect_once(endpoint, api_key)
        .await
        .map_err(QwenRealtimeASRError::ConnectionFailed)
}

async fn connect_once(endpoint: &str, api_key: &str) -> Result<WsStream, String> {
    let mut request = endpoint.into_client_request().map_err(|e| e.to_string())?;
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|e| e.to_string())?,
    );
    request
        .headers_mut()
        .insert("OpenAI-Beta", HeaderValue::from_static("realtime=v1"));

    connect_async(request)
        .await
        .map(|(ws, _)| ws)
        .map_err(|e| e.to_string())
}

fn is_retryable_connection_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "tls handshake eof",
        "handshake eof",
        "unexpected eof",
        "connection reset",
        "connection aborted",
        "os error 10054",
        "timed out",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

fn session_update_message() -> String {
    json!({
        "event_id": new_event_id(),
        "type": "session.update",
        "session": {
            "modalities": ["text"],
            "input_audio_format": "pcm",
            "sample_rate": 16000,
            "input_audio_transcription": { "language": "zh" },
            "turn_detection": {
                "type": "server_vad",
                "threshold": 0.0,
                "silence_duration_ms": 400
            }
        }
    })
    .to_string()
}

fn append_audio_message(data: &[u8]) -> String {
    json!({
        "event_id": new_event_id(),
        "type": "input_audio_buffer.append",
        "audio": encode_base64_standard(data)
    })
    .to_string()
}

fn session_finish_message() -> String {
    json!({
        "event_id": new_event_id(),
        "type": "session.finish"
    })
    .to_string()
}

fn new_event_id() -> String {
    format!("event_{}", Uuid::new_v4().simple())
}

fn encode_base64_standard(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn extract_transcript_text(value: &Value) -> Option<&str> {
    value
        .get("transcript")
        .and_then(Value::as_str)
        .or_else(|| value.get("text").and_then(Value::as_str))
        .or_else(|| value.get("stash").and_then(Value::as_str))
}

async fn send_text(writer: &SharedWriter, text: String) -> Result<(), QwenRealtimeASRError> {
    let mut guard = writer.lock().await;
    let Some(sink) = guard.as_mut() else {
        return Err(QwenRealtimeASRError::ConnectionFailed(
            "websocket writer closed".to_string(),
        ));
    };
    tokio::time::timeout(Duration::from_secs(5), sink.send(Message::Text(text)))
        .await
        .map_err(|_| QwenRealtimeASRError::SendFailed("websocket send timed out".to_string()))?
        .map_err(|e| QwenRealtimeASRError::SendFailed(e.to_string()))
}

async fn close_writer(writer: &SharedWriter) -> Result<(), QwenRealtimeASRError> {
    if let Some(mut sink) = writer.lock().await.take() {
        sink.close()
            .await
            .map_err(|e| QwenRealtimeASRError::SendFailed(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn audio_bytes_are_ordered_and_complete_before_unique_finish() {
        for delayed_start in [false, true] {
            for sizes in [
                vec![2],
                vec![3198],
                vec![3200],
                vec![3202],
                vec![6240],
                vec![42, 3110, 3088],
            ] {
                let asr = Arc::new(QwenRealtimeASR::new(QwenRealtimeCredentials {
                    api_key: "test".into(),
                    endpoint: String::new(),
                    model: String::new(),
                }));
                let (tx, mut rx) = mpsc::unbounded_channel();
                asr.state.lock().send_tx = Some(tx);
                if !delayed_start {
                    asr.mark_session_started();
                }
                let mut expected = Vec::new();
                for size in sizes {
                    let offset = expected.len();
                    let bytes: Vec<u8> = (0..size).map(|i| ((offset + i) % 251) as u8).collect();
                    asr.consume_pcm_chunk(&bytes);
                    expected.extend(bytes);
                }
                if delayed_start {
                    asr.mark_session_started();
                }
                let finishing = {
                    let asr = Arc::clone(&asr);
                    tokio::spawn(async move { asr.send_last_frame().await })
                };
                let mut actual = Vec::new();
                loop {
                    match rx.recv().await.unwrap() {
                        SendItem::Audio(bytes) => actual.extend(bytes),
                        SendItem::Finish(done) => {
                            done.send(Ok(())).unwrap();
                            break;
                        }
                    }
                }
                finishing.await.unwrap().unwrap();
                assert_eq!(actual, expected);
                asr.consume_pcm_chunk(&[99, 100]);
                assert!(rx.try_recv().is_err());
                assert!(asr.send_last_frame().await.is_err());
                asr.cancel();
            }
        }
    }

    #[tokio::test]
    async fn cancel_interrupts_pending_finish_ack_and_rejects_late_audio() {
        let asr = Arc::new(QwenRealtimeASR::new(QwenRealtimeCredentials {
            api_key: "test".into(),
            endpoint: String::new(),
            model: String::new(),
        }));
        let (tx, mut rx) = mpsc::unbounded_channel();
        asr.state.lock().send_tx = Some(tx);
        asr.mark_session_started();
        let (received_tx, received_rx) = oneshot::channel();
        *asr.send_task.lock() = Some(tokio::spawn(async move {
            if let Some(SendItem::Finish(_ack)) = rx.recv().await {
                let _ = received_tx.send(());
                std::future::pending::<()>().await;
            }
        }));
        let finishing = {
            let asr = Arc::clone(&asr);
            tokio::spawn(async move { asr.send_last_frame().await })
        };
        received_rx.await.unwrap();
        asr.cancel();
        assert!(tokio::time::timeout(Duration::from_secs(1), finishing)
            .await
            .unwrap()
            .unwrap()
            .is_err());
        asr.consume_pcm_chunk(&[1, 2]);
        assert_eq!(asr.state.lock().bytes_received, 0);
    }

    #[test]
    fn qwen_realtime_uses_builtin_stable_model_and_endpoint() {
        let preset = qwen_realtime_preset();

        assert_eq!(preset.model, "qwen3-asr-flash-realtime");
        assert_eq!(
            preset.endpoint_cn,
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime"
        );
    }

    #[test]
    fn realtime_endpoint_with_model_appends_model_query() {
        let endpoint = realtime_endpoint_with_model(
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime",
            "qwen3-asr-flash-realtime",
        )
        .unwrap();

        assert_eq!(
            endpoint,
            "wss://dashscope.aliyuncs.com/api-ws/v1/realtime?model=qwen3-asr-flash-realtime"
        );
    }

    #[test]
    fn transient_transport_errors_are_retried_but_auth_errors_are_not() {
        assert!(is_retryable_connection_error("IO error: tls handshake eof"));
        assert!(is_retryable_connection_error(
            "IO error: connection reset by peer (os error 10054)"
        ));
        assert!(!is_retryable_connection_error(
            "HTTP error: 401 Unauthorized"
        ));
        assert!(!is_retryable_connection_error("InvalidApiKey"));
    }

    #[test]
    fn session_update_uses_pcm16_chinese_and_server_vad() {
        let message = session_update_message();
        let json: serde_json::Value = serde_json::from_str(&message).unwrap();

        assert_eq!(json["type"], "session.update");
        assert_eq!(json["session"]["input_audio_format"], "pcm");
        assert_eq!(
            json["session"]["input_audio_transcription"]["language"],
            "zh"
        );
        assert_eq!(json["session"]["turn_detection"]["type"], "server_vad");
    }

    #[test]
    fn append_audio_message_encodes_pcm_as_base64() {
        let message = append_audio_message(&[0, 1, 2, 3]);
        let json: serde_json::Value = serde_json::from_str(&message).unwrap();

        assert_eq!(json["type"], "input_audio_buffer.append");
        assert_eq!(json["audio"], "AAECAw==");
    }

    #[test]
    fn extract_transcript_accepts_final_delta_shapes() {
        let value = serde_json::json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "transcript": "DeepSeek stream test"
        });
        let text = extract_transcript_text(&value);

        assert_eq!(text, Some("DeepSeek stream test"));
    }

    #[test]
    fn session_finished_transcript_does_not_drop_accumulated_completed_segments() {
        let asr = QwenRealtimeASR::new(QwenRealtimeCredentials {
            api_key: "test-key".to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            model: DEFAULT_MODEL.to_string(),
        });
        let (tx, mut rx) = oneshot::channel();
        {
            let mut st = asr.state.lock();
            st.final_tx = Some(tx);
            st.bytes_received = 64_000;
        }

        assert!(asr.handle_text_message(
            r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"第一段。"}"#
        ));
        assert!(asr.handle_text_message(
            r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"第二段。"}"#
        ));
        assert!(!asr.handle_text_message(r#"{"type":"session.finished","transcript":"第一段。"}"#));

        let raw = rx.try_recv().unwrap().unwrap();
        assert_eq!(raw.text, "第一段。第二段。");
    }

    #[test]
    fn session_finished_transcript_is_used_when_no_completed_segments_exist() {
        let asr = QwenRealtimeASR::new(QwenRealtimeCredentials {
            api_key: "test-key".to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            model: DEFAULT_MODEL.to_string(),
        });
        let (tx, mut rx) = oneshot::channel();
        {
            let mut st = asr.state.lock();
            st.final_tx = Some(tx);
            st.bytes_received = 32_000;
        }

        assert!(!asr
            .handle_text_message(r#"{"type":"session.finished","transcript":"结束事件文本。"}"#));

        let raw = rx.try_recv().unwrap().unwrap();
        assert_eq!(raw.text, "结束事件文本。");
    }
}
