//! Batch Whisper ASR client — collects PCM in a buffer, then POSTs a WAV file
//! to any OpenAI-compatible `/audio/transcriptions` endpoint on session end.

use anyhow::{Context, Result};
use parking_lot::Mutex;

use crate::asr::wav::encode_wav_16k_mono;
use crate::asr::RawTranscript;

/// Whisper の `prompt` パラメータの安全側上限（文字数）。
///
/// OpenAI / Groq の Audio Transcriptions API は `prompt` を 244 トークンまで
/// 受け付ける。トークナイザは BPE で言語によって 1 token あたりの文字数が
/// 異なる：英語は ~4 chars/token、日本語・中国語は最悪 ~1 char/token。
/// CJK ユーザーが安全に収まるよう、文字数で 240 を上限にする。
pub const PROMPT_CHAR_BUDGET: usize = 240;

/// 区切り文字（ASCII）。Whisper のトークナイザはどの言語でも安定して扱える。
const PROMPT_SEPARATOR: &str = ", ";

/// Normalize a user-supplied OpenAI-compatible ASR endpoint to the
/// transcription resource. Both a service base URL and the exact
/// `/audio/transcriptions` URL are accepted so copied provider examples work
/// without producing a duplicated path.
pub fn transcriptions_url(base_url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(base_url.trim()).map_err(|_| "endpointInvalid".to_string())?;
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() != "https" && !is_loopback_host(host) {
        return Err("endpointMustUseHttps".to_string());
    }

    // Work on the URL path only so query parameters stay intact.
    let mut url = parsed.clone();
    let path = parsed.path().trim_end_matches('/');
    let next_path = if path.ends_with("/audio/transcriptions") {
        path.to_string()
    } else if path.ends_with("/audio") {
        format!("{path}/transcriptions")
    } else if let Some(prefix) = path.strip_suffix("/chat/completions") {
        format!("{prefix}/audio/transcriptions")
    } else {
        format!("{path}/audio/transcriptions")
    };
    url.set_path(&next_path);
    Ok(url.to_string())
}

/// Local OpenAI-compatible ASR servers commonly have no authentication. A
/// remote endpoint remains key-protected by default, including malformed URLs
/// so validation fails closed instead of treating an unknown host as local.
pub fn endpoint_requires_api_key(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url.trim()) else {
        return true;
    };
    !is_loopback_host(url.host_str().unwrap_or_default())
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host == "[::1]"
}

pub struct WhisperBatchASR {
    api_key: String,
    base_url: String,
    model: String,
    /// 任意のプロンプト（語彙ヒント等）。空文字や空白のみは送信しない。
    /// `None` ＝ プロンプト無し（既存挙動）。
    prompt: Option<String>,
    buffer: Mutex<Vec<u8>>,
}

impl WhisperBatchASR {
    pub fn new(api_key: String, base_url: String, model: String, prompt: Option<String>) -> Self {
        Self {
            api_key,
            base_url,
            model,
            prompt,
            buffer: Mutex::new(Vec::new()),
        }
    }

    /// Stop collecting audio, encode the buffer as WAV, and POST to the
    /// Whisper transcriptions endpoint.
    ///
    /// 失败时**保留** PCM buffer，让上层有机会重试或在历史中至少留一个失败记录；
    /// 之前的实现一进函数就 `mem::take` 把 buffer 清空，凭证错或网络中断都会
    /// 让用户的录音直接消失。
    pub async fn transcribe(&self) -> Result<RawTranscript> {
        // clone 而不是 take：~30s 16 kHz 16-bit 音频 ≈ 960 KB，会话末调用一次，可接受。
        let pcm = self.buffer.lock().clone();
        if pcm.is_empty() {
            return Ok(RawTranscript {
                text: String::new(),
                duration_ms: 0,
            });
        }

        let result = self.transcribe_inner(&pcm).await;
        // 仅在成功路径上才清 buffer。失败时 PCM 还在，coordinator 拿到 Err 但
        // 用户重新触发 stop 时仍能再发一次，或日后增加重试入口时复用。
        if result.is_ok() {
            self.buffer.lock().clear();
        }
        result
    }

    async fn transcribe_inner(&self, pcm: &[u8]) -> Result<RawTranscript> {
        // 16 kHz mono 16-bit: 2 bytes per sample.
        let duration_ms = (pcm.len() as u64 / 2) * 1000 / 16_000;

        if endpoint_requires_api_key(&self.base_url) && self.api_key.trim().is_empty() {
            anyhow::bail!("Whisper API key missing");
        }

        let samples: Vec<i16> = pcm
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let wav = encode_wav_16k_mono(&samples);
        let url = transcriptions_url(&self.base_url).map_err(anyhow::Error::msg)?;

        let wav_part = reqwest::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .context("set MIME type")?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", wav_part)
            .text("model", self.model.clone());

        // `prompt` は空文字を送らない：OpenAI 互換実装によっては空文字でエラーに
        // なるリスクがある（Groq は許容するが防御的にスキップ）。`trim()` で
        // 空白のみのケースも除外。
        if let Some(prompt) = self.prompt.as_ref() {
            let trimmed = prompt.trim();
            if !trimmed.is_empty() {
                form = form.text("prompt", trimmed.to_string());
            }
        }

        let client = reqwest::Client::new();
        let mut request = client.post(&url);
        if !self.api_key.trim().is_empty() {
            request = request.header("Authorization", format!("Bearer {}", self.api_key.trim()));
        }
        let resp = request
            .multipart(form)
            .send()
            .await
            .context("Whisper HTTP request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Whisper API error {}: {}", status, body);
        }

        let json: serde_json::Value = resp.json().await.context("parse Whisper response")?;
        let text = json["text"].as_str().unwrap_or("").trim().to_string();

        Ok(RawTranscript { text, duration_ms })
    }

    pub fn cancel(&self) {
        self.buffer.lock().clear();
    }
}

impl crate::recorder::AudioConsumer for WhisperBatchASR {
    fn consume_pcm_chunk(&self, pcm: &[u8]) {
        self.buffer.lock().extend_from_slice(pcm);
    }
}

/// 用户辞書の有効フレーズから Whisper の `prompt` パラメータを組み立てる。
///
/// Whisper は `prompt` で語彙ヒント / スタイル文脈を渡せる：固有名詞・専門
/// 用語の表記揺れを抑え、ASR 段階で正しい綴り（漢字選択を含む）に偏らせる。
/// 既存の dictionary 機能はこれまで Volcengine ASR と Polish LLM のみに渡って
/// いて、Whisper 互換プロバイダ（whisper / siliconflow / zhipu / groq）には
/// 流れていなかった。本関数で同じエントリを Whisper にも届ける。
///
/// # 仕様
///
/// - 空白のみのフレーズは除外
/// - 区切りは `, `
/// - 末尾に `.` を付与して「文の終わり」を Whisper に明示（モデルがプロンプト
///   を続きと誤解して書き起こし冒頭に混入するのを抑える）
/// - 文字数が `PROMPT_CHAR_BUDGET` を超えるエントリは**スキップ**して次に
///   進む（途中で打ち切らない）。これにより「先頭に長文 1 件があると残りが
///   全部捨てられる」現象を回避でき、登録順を保ちつつ収まるエントリを最大化
///   できる。
/// - 入力が空、または有効フレーズが 0 件の場合は `None` を返す。Optional に
///   することで「プロンプト無し」と「空文字プロンプト」を呼び出し側で区別
///   する必要をなくす。
pub fn build_prompt_from_phrases(phrases: &[String]) -> Option<String> {
    let mut included: Vec<&str> = Vec::new();
    let mut total_chars: usize = 0;

    for phrase in phrases {
        let trimmed = phrase.trim();
        if trimmed.is_empty() {
            continue;
        }
        let phrase_chars = trimmed.chars().count();
        let added = if included.is_empty() {
            phrase_chars
        } else {
            PROMPT_SEPARATOR.chars().count() + phrase_chars
        };
        // 末尾の "." 1 文字も予約。
        if total_chars + added + 1 > PROMPT_CHAR_BUDGET {
            continue;
        }
        included.push(trimmed);
        total_chars += added;
    }

    if included.is_empty() {
        return None;
    }
    let mut s = included.join(PROMPT_SEPARATOR);
    s.push('.');
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcriptions_url_accepts_a_base_or_exact_endpoint() {
        assert_eq!(
            transcriptions_url("https://api.siliconflow.cn/v1").unwrap(),
            "https://api.siliconflow.cn/v1/audio/transcriptions"
        );
        assert_eq!(
            transcriptions_url("https://api.siliconflow.cn/v1/audio/transcriptions").unwrap(),
            "https://api.siliconflow.cn/v1/audio/transcriptions"
        );
        assert_eq!(
            transcriptions_url("http://127.0.0.1:8000/v1?profile=local").unwrap(),
            "http://127.0.0.1:8000/v1/audio/transcriptions?profile=local"
        );
    }

    #[test]
    fn only_loopback_endpoints_can_omit_an_api_key() {
        assert!(!endpoint_requires_api_key("http://localhost:8000/v1"));
        assert!(!endpoint_requires_api_key("http://127.0.0.1:8000/v1"));
        assert!(!endpoint_requires_api_key("http://[::1]:8000/v1"));
        assert!(endpoint_requires_api_key("https://api.siliconflow.cn/v1"));
        assert!(endpoint_requires_api_key("not a url"));
    }

    #[test]
    fn build_prompt_returns_none_for_empty_input() {
        assert_eq!(build_prompt_from_phrases(&[]), None);
    }

    #[test]
    fn build_prompt_returns_none_when_all_phrases_blank() {
        let phrases = vec!["".to_string(), "   ".to_string(), "\t\n".to_string()];
        assert_eq!(build_prompt_from_phrases(&phrases), None);
    }

    #[test]
    fn build_prompt_single_phrase() {
        let phrases = vec!["梁山泊".to_string()];
        assert_eq!(
            build_prompt_from_phrases(&phrases),
            Some("梁山泊.".to_string())
        );
    }

    #[test]
    fn build_prompt_joins_with_comma_and_appends_period() {
        let phrases = vec![
            "梁山泊".to_string(),
            "片沼ほとり".to_string(),
            "TRC".to_string(),
        ];
        assert_eq!(
            build_prompt_from_phrases(&phrases),
            Some("梁山泊, 片沼ほとり, TRC.".to_string())
        );
    }

    #[test]
    fn build_prompt_trims_each_phrase() {
        let phrases = vec!["  梁山泊  ".to_string(), "\tTRC\n".to_string()];
        assert_eq!(
            build_prompt_from_phrases(&phrases),
            Some("梁山泊, TRC.".to_string())
        );
    }

    #[test]
    fn build_prompt_skips_blank_entries_in_middle() {
        let phrases = vec![
            "alpha".to_string(),
            "".to_string(),
            "   ".to_string(),
            "beta".to_string(),
        ];
        assert_eq!(
            build_prompt_from_phrases(&phrases),
            Some("alpha, beta.".to_string())
        );
    }

    #[test]
    fn build_prompt_truncates_overflow_but_keeps_short_entries_after_long_one() {
        // 先頭に 250 文字の長文 → 単独で予算超過 → スキップ。続く短いエントリは
        // 採用される。「途中で break しない」契約の検証。
        let long = "あ".repeat(250);
        let phrases = vec![long.clone(), "梁山泊".to_string(), "TRC".to_string()];
        let prompt = build_prompt_from_phrases(&phrases).expect("non-empty");
        assert!(!prompt.contains(&long), "long phrase must be dropped");
        assert!(prompt.contains("梁山泊"));
        assert!(prompt.contains("TRC"));
        assert!(prompt.ends_with('.'));
    }

    #[test]
    fn build_prompt_respects_char_budget() {
        // 6 文字 × 50 件 = 300 文字（区切り込みでさらに増える）→ 予算超過分は捨てる。
        let phrases: Vec<String> = (0..50).map(|i| format!("word{:02}", i)).collect();
        let prompt = build_prompt_from_phrases(&phrases).expect("non-empty");
        assert!(
            prompt.chars().count() <= PROMPT_CHAR_BUDGET,
            "prompt length {} exceeds budget {}",
            prompt.chars().count(),
            PROMPT_CHAR_BUDGET
        );
        assert!(prompt.ends_with('.'));
    }

    #[test]
    fn build_prompt_includes_first_entries_when_truncating_in_order() {
        // 順序保証：登録順の早いものから入る。後続が落ちる。
        let phrases: Vec<String> = (0..100).map(|i| format!("entry{:03}", i)).collect();
        let prompt = build_prompt_from_phrases(&phrases).expect("non-empty");
        assert!(prompt.contains("entry000"));
        assert!(prompt.contains("entry001"));
        // 100 件 × 8 文字以上は確実に予算超過 → 末尾は入らない
        assert!(!prompt.contains("entry099"));
    }
}
