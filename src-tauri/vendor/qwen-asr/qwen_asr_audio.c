/*
 * qwen_asr_audio.c - WAV loading and mel spectrogram computation
 *
 * Mel spectrogram parameters (WhisperFeatureExtractor):
 *   Sample rate: 16000 Hz
 *   Mel bins: 128
 *   Hop length: 160 (10ms)
 *   Window size: 400 (25ms)
 *
 * Key difference from Voxtral: uses dynamic maximum for clamping
 * instead of a fixed global_log_mel_max = 1.5.
 */

#include "qwen_asr_audio.h"
#include "qwen_asr_kernels.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <limits.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define SAMPLE_RATE  16000
#define N_MEL        128
#define HOP_LENGTH   160
#define WIN_LENGTH   400
#define N_FFT        400
#define N_FREQ       (N_FFT / 2 + 1)    /* 201 bins */

/* ========================================================================
 * WAV File Loading (adapted from voxtral)
 * ======================================================================== */

static uint16_t read_u16(const uint8_t *p) { return p[0] | (p[1] << 8); }
static uint32_t read_u32(const uint8_t *p) { return p[0] | (p[1] << 8) | (p[2] << 16) | (p[3] << 24); }

float *qwen_parse_wav_buffer(const uint8_t *data, size_t file_size, int *out_n_samples) {
    if (file_size < 44 || memcmp(data, "RIFF", 4) != 0 || memcmp(data + 8, "WAVE", 4) != 0) {
        fprintf(stderr, "parse_wav_buffer: not a valid WAV file\n");
        return NULL;
    }

    int channels = 0, sample_rate = 0, bits_per_sample = 0;
    int audio_format = 0;
    const uint8_t *pcm_data = NULL;
    int pcm_size = 0;

    const uint8_t *p = data + 12;
    const uint8_t *end = data + file_size;

    while (p + 8 <= end) {
        uint32_t chunk_size = read_u32(p + 4);
        if (p + 8 + chunk_size > end) break;
        if (memcmp(p, "fmt ", 4) == 0 && chunk_size >= 16) {
            audio_format = read_u16(p + 8);
            channels = read_u16(p + 10);
            sample_rate = read_u32(p + 12);
            bits_per_sample = read_u16(p + 22);
        } else if (memcmp(p, "data", 4) == 0) {
            pcm_data = p + 8;
            pcm_size = chunk_size;
            if (pcm_data + pcm_size > end) pcm_size = (int)(end - pcm_data);
        }
        p += 8 + chunk_size;
        if (chunk_size & 1) p++;
    }

    if (audio_format != 1 || bits_per_sample != 16 || pcm_data == NULL || channels < 1) {
        fprintf(stderr, "parse_wav_buffer: unsupported format (need 16-bit PCM, got fmt=%d bits=%d)\n",
                audio_format, bits_per_sample);
        return NULL;
    }

    int n_frames = pcm_size / (channels * 2);
    float *samples = (float *)malloc(n_frames * sizeof(float));
    if (!samples) return NULL;

    const int16_t *src = (const int16_t *)pcm_data;
    for (int i = 0; i < n_frames; i++) {
        if (channels == 1) {
            samples[i] = src[i] / 32768.0f;
        } else {
            float sum = 0;
            for (int c = 0; c < channels; c++) {
                int16_t val;
                memcpy(&val, &src[i * channels + c], sizeof(int16_t));
                sum += val;
            }
            samples[i] = (sum / channels) / 32768.0f;
        }
    }

    /* Resample to 16kHz if needed — windowed-sinc interpolation with
     * Kaiser window for proper anti-aliasing when downsampling. */
    if (sample_rate != SAMPLE_RATE) {
        int new_n = (int)((long long)n_frames * SAMPLE_RATE / sample_rate);
        float *resampled = (float *)malloc(new_n * sizeof(float));
        if (!resampled) { free(samples); return NULL; }

        /* Sinc resampler parameters */
        const int SINC_HALF = 16;          /* zero-crossings per side */
        const double KAISER_BETA = 6.0;    /* sidelobe suppression */
        /* Cutoff at the lower Nyquist to prevent aliasing */
        double ratio = (double)SAMPLE_RATE / (double)sample_rate;
        double cutoff = (ratio < 1.0) ? ratio : 1.0;

        /* Precompute Kaiser window: I0(beta * sqrt(1 - (n/N)^2)) / I0(beta)
         * I0 approximation via power series (converges fast for beta <= 10). */
        /* I0 (modified Bessel, first kind, order 0) */
        #define BESSEL_I0(x) ({ \
            double _sum = 1.0, _term = 1.0, _xx = (x)*(x); \
            for (int _k = 1; _k <= 20; _k++) { \
                _term *= _xx / (4.0 * (double)_k * (double)_k); \
                _sum += _term; \
            } \
            _sum; })
        double inv_I0_beta = 1.0 / BESSEL_I0(KAISER_BETA);

        for (int i = 0; i < new_n; i++) {
            double src_pos = (double)i / ratio;
            int center = (int)src_pos;
            double acc = 0.0;
            double wsum = 0.0;

            int j_lo = center - SINC_HALF + 1;
            int j_hi = center + SINC_HALF;
            for (int j = j_lo; j <= j_hi; j++) {
                double d = (double)j - src_pos;        /* distance in source samples */
                double x = d * cutoff;                  /* scale by cutoff */

                /* Sinc value */
                double s;
                if (fabs(x) < 1e-9) {
                    s = 1.0;
                } else {
                    s = sin(M_PI * x) / (M_PI * x);
                }

                /* Kaiser window over the support [-SINC_HALF, SINC_HALF] */
                double npos = d / SINC_HALF;  /* normalized to [-1, 1] */
                double w;
                if (npos <= -1.0 || npos >= 1.0) {
                    w = 0.0;
                } else {
                    w = BESSEL_I0(KAISER_BETA * sqrt(1.0 - npos * npos)) * inv_I0_beta;
                }

                double coeff = s * w * cutoff;
                if (j >= 0 && j < n_frames) {
                    acc += samples[j] * coeff;
                }
                wsum += coeff;
            }
            /* Normalize to handle edge effects at boundaries */
            resampled[i] = (wsum > 1e-9) ? (float)(acc / wsum) : 0.0f;
        }
        #undef BESSEL_I0
        free(samples);
        samples = resampled;
        n_frames = new_n;
    }

    *out_n_samples = n_frames;
    return samples;
}

float *qwen_load_wav(const char *path, int *out_n_samples) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "qwen_load_wav: cannot open %s\n", path);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long file_size = ftell(f);
    if (file_size <= 0) { fclose(f); return NULL; }
    fseek(f, 0, SEEK_SET);
    uint8_t *data = (uint8_t *)malloc(file_size);
    if (!data || fread(data, 1, file_size, f) != (size_t)file_size) {
        fclose(f); free(data); return NULL;
    }
    fclose(f);
    float *samples = qwen_parse_wav_buffer(data, (size_t)file_size, out_n_samples);
    free(data);
    return samples;
}

float *qwen_read_pcm_stdin(int *out_n_samples) {
    size_t capacity = 1024 * 1024;
    size_t size = 0;
    uint8_t *buf = (uint8_t *)malloc(capacity);
    if (!buf) return NULL;
    while (1) {
        if (size == capacity) {
            capacity *= 2;
            uint8_t *tmp = (uint8_t *)realloc(buf, capacity);
            if (!tmp) { free(buf); return NULL; }
            buf = tmp;
        }
        size_t n = fread(buf + size, 1, capacity - size, stdin);
        if (n == 0) break;
        size += n;
    }
    if (size < 4) {
        fprintf(stderr, "qwen_read_pcm_stdin: no data on stdin\n");
        free(buf); return NULL;
    }
    if (qwen_verbose >= 2)
        fprintf(stderr, "Read %zu bytes from stdin\n", size);
    if (memcmp(buf, "RIFF", 4) == 0) {
        if (qwen_verbose >= 2)
            fprintf(stderr, "Detected WAV format on stdin\n");
        float *samples = qwen_parse_wav_buffer(buf, size, out_n_samples);
        free(buf);
        return samples;
    }
    /* Raw s16le 16kHz mono */
    if (qwen_verbose >= 2)
        fprintf(stderr, "Treating stdin as raw s16le 16kHz mono\n");
    int n_frames = (int)(size / 2);
    float *samples = (float *)malloc(n_frames * sizeof(float));
    if (!samples) { free(buf); return NULL; }
    const int16_t *src = (const int16_t *)buf;
    for (int i = 0; i < n_frames; i++) samples[i] = src[i] / 32768.0f;
    free(buf);
    *out_n_samples = n_frames;
    return samples;
}

/* ========================================================================
 * Mel Filter Bank (Slaney-style)
 * ======================================================================== */

static float hertz_to_mel(float freq) {
    const float min_log_hertz = 1000.0f;
    const float min_log_mel = 15.0f;
    const float logstep = 27.0f / logf(6.4f);
    float mels = 3.0f * freq / 200.0f;
    if (freq >= min_log_hertz) mels = min_log_mel + logf(freq / min_log_hertz) * logstep;
    return mels;
}

static float mel_to_hertz(float mels) {
    const float min_log_hertz = 1000.0f;
    const float min_log_mel = 15.0f;
    const float logstep = logf(6.4f) / 27.0f;
    float freq = 200.0f * mels / 3.0f;
    if (mels >= min_log_mel) freq = min_log_hertz * expf(logstep * (mels - min_log_mel));
    return freq;
}

static float *build_mel_filters(void) {
    float *filters = (float *)calloc((size_t)N_MEL * N_FREQ, sizeof(float));
    if (!filters) return NULL;

    float fft_freqs[N_FREQ];
    for (int i = 0; i < N_FREQ; i++)
        fft_freqs[i] = (float)i * ((float)SAMPLE_RATE / 2.0f) / (float)(N_FREQ - 1);

    float mel_min = hertz_to_mel(0.0f);
    float mel_max = hertz_to_mel((float)SAMPLE_RATE / 2.0f);

    float filter_freqs[N_MEL + 2];
    float filter_diff[N_MEL + 1];
    for (int i = 0; i < N_MEL + 2; i++) {
        float mel = mel_min + (mel_max - mel_min) * (float)i / (float)(N_MEL + 1);
        filter_freqs[i] = mel_to_hertz(mel);
    }
    for (int i = 0; i < N_MEL + 1; i++) {
        filter_diff[i] = filter_freqs[i + 1] - filter_freqs[i];
        if (filter_diff[i] == 0.0f) filter_diff[i] = 1e-6f;
    }

    for (int m = 0; m < N_MEL; m++) {
        float enorm = 2.0f / (filter_freqs[m + 2] - filter_freqs[m]);
        for (int f = 0; f < N_FREQ; f++) {
            float down = (fft_freqs[f] - filter_freqs[m]) / filter_diff[m];
            float up = (filter_freqs[m + 2] - fft_freqs[f]) / filter_diff[m + 1];
            float val = fminf(down, up);
            if (val < 0.0f) val = 0.0f;
            filters[(size_t)m * N_FREQ + f] = val * enorm;
        }
    }
    return filters;
}

/* ========================================================================
 * Mel Spectrogram (dynamic max, returns [128, n_frames])
 * ======================================================================== */

float *qwen_mel_spectrogram(const float *samples, int n_samples, int *out_frames) {
    int n_fft = N_FFT;
    int n_freqs = N_FREQ;
    int pad_len = n_fft / 2; /* center=True padding (reflect) */

    /* Reflect-pad the signal */
    int padded_len = n_samples + 2 * pad_len;
    float *padded = (float *)malloc(padded_len * sizeof(float));
    for (int i = 0; i < pad_len; i++) {
        int src = pad_len - i;
        padded[i] = (src < n_samples) ? samples[src] : 0.0f;
    }
    memcpy(padded + pad_len, samples, n_samples * sizeof(float));
    for (int i = 0; i < pad_len; i++) {
        int src = n_samples - 2 - i;
        padded[pad_len + n_samples + i] = (src >= 0) ? samples[src] : 0.0f;
    }

    int n_frames_total = (padded_len - n_fft) / HOP_LENGTH + 1;
    int n_frames = n_frames_total - 1; /* drop last frame */
    if (n_frames <= 0) {
        fprintf(stderr, "qwen_mel_spectrogram: audio too short (%d samples)\n", n_samples);
        free(padded);
        return NULL;
    }

    float *mel_filters = build_mel_filters();
    if (!mel_filters) { free(padded); return NULL; }

    /* Periodic Hann window */
    float window[WIN_LENGTH];
    for (int i = 0; i < WIN_LENGTH; i++)
        window[i] = 0.5f * (1.0f - cosf(2.0f * (float)M_PI * (float)i / (float)WIN_LENGTH));

    /* Precompute DFT tables */
    float *dft_cos = (float *)malloc((size_t)N_FREQ * N_FFT * sizeof(float));
    float *dft_sin = (float *)malloc((size_t)N_FREQ * N_FFT * sizeof(float));
    for (int k = 0; k < N_FREQ; k++) {
        for (int n = 0; n < N_FFT; n++) {
            float angle = 2.0f * (float)M_PI * (float)k * (float)n / (float)N_FFT;
            dft_cos[k * N_FFT + n] = cosf(angle);
            dft_sin[k * N_FFT + n] = sinf(angle);
        }
    }

    /* First pass: compute mel values and find global max.
     * Store as [n_frames, N_MEL] temporarily for convenient max search. */
    float *mel_tmp = (float *)calloc(n_frames * N_MEL, sizeof(float));
    float windowed[N_FFT];
    float power[N_FREQ];
    float global_max = -1e30f;

    for (int t = 0; t < n_frames; t++) {
        int start = t * HOP_LENGTH;
        for (int i = 0; i < N_FFT; i++)
            windowed[i] = padded[start + i] * window[i];

        for (int k = 0; k < n_freqs; k++) {
            float re = 0, im = 0;
            const float *cos_row = dft_cos + k * N_FFT;
            const float *sin_row = dft_sin + k * N_FFT;
            for (int n = 0; n < N_FFT; n++) {
                re += windowed[n] * cos_row[n];
                im += windowed[n] * sin_row[n];
            }
            power[k] = re * re + im * im;
        }

        for (int m = 0; m < N_MEL; m++) {
            float sum = 0.0f;
            const float *filt = mel_filters + (size_t)m * n_freqs;
            for (int k = 0; k < n_freqs; k++) sum += filt[k] * power[k];
            if (sum < 1e-10f) sum = 1e-10f;
            float val = log10f(sum);
            mel_tmp[t * N_MEL + m] = val;
            if (val > global_max) global_max = val;
        }
    }

    /* Second pass: clamp with dynamic max and normalize.
     * Output layout: [N_MEL, n_frames] for Conv2D compatibility. */
    float *mel = (float *)malloc((size_t)N_MEL * n_frames * sizeof(float));
    float min_val = global_max - 8.0f;

    for (int t = 0; t < n_frames; t++) {
        for (int m = 0; m < N_MEL; m++) {
            float val = mel_tmp[t * N_MEL + m];
            if (val < min_val) val = min_val;
            /* Store as [mel_bin, frame] for Conv2D input */
            mel[m * n_frames + t] = (val + 4.0f) / 4.0f;
        }
    }

    free(mel_tmp);
    free(dft_cos);
    free(dft_sin);
    free(padded);
    free(mel_filters);

    *out_frames = n_frames;
    return mel;
}

/* ========================================================================
 * Live Audio: stdin reader thread for incremental streaming
 * ======================================================================== */

#include <pthread.h>

/* Append n_new float samples to la->samples under mutex + signal condvar. */
static void live_audio_append(qwen_live_audio_t *la, const float *data, int n_new) {
    if (!la || !data || n_new <= 0) return;

    pthread_mutex_lock(&la->mutex);
    int64_t need = la->n_samples + (int64_t)n_new;
    if (need > la->capacity) {
        int64_t new_cap = la->capacity > 0 ? la->capacity : 32000;
        while (new_cap < need) new_cap *= 2;
        if ((uint64_t)new_cap > (uint64_t)(SIZE_MAX / sizeof(float))) {
            pthread_mutex_unlock(&la->mutex);
            return;
        }
        float *tmp = (float *)realloc(la->samples, (size_t)new_cap * sizeof(float));
        if (!tmp) {
            pthread_mutex_unlock(&la->mutex);
            return;
        }
        la->samples = tmp;
        la->capacity = new_cap;
    }
    memcpy(la->samples + (size_t)la->n_samples, data, (size_t)n_new * sizeof(float));
    la->n_samples += n_new;
    pthread_cond_signal(&la->cond);
    pthread_mutex_unlock(&la->mutex);
}

/* Convert a chunk of s16le bytes to float samples and append. */
static void live_audio_convert_and_append(qwen_live_audio_t *la,
                                          const uint8_t *buf, size_t n_bytes) {
    int n_frames = (int)(n_bytes / 2);
    if (n_frames <= 0) return;
    float *tmp = (float *)malloc((size_t)n_frames * sizeof(float));
    if (!tmp) return;
    const int16_t *src = (const int16_t *)buf;
    for (int i = 0; i < n_frames; i++) tmp[i] = src[i] / 32768.0f;
    live_audio_append(la, tmp, n_frames);
    free(tmp);
}

typedef struct {
    qwen_live_audio_t *la;
    int is_wav;
    int data_remaining;  /* bytes remaining in WAV data chunk, -1 if raw */
} live_reader_ctx_t;

static void *live_reader_thread(void *arg) {
    live_reader_ctx_t *rctx = (live_reader_ctx_t *)arg;
    qwen_live_audio_t *la = rctx->la;
    int is_wav = rctx->is_wav;
    int data_remaining = rctx->data_remaining;
    free(rctx);

    /* Read stdin in ~2s chunks: 32000 samples * 2 bytes = 64000 bytes */
    const size_t READ_SIZE = 64000;
    uint8_t *buf = (uint8_t *)malloc(READ_SIZE);
    if (!buf) {
        pthread_mutex_lock(&la->mutex);
        la->eof = 1;
        pthread_cond_signal(&la->cond);
        pthread_mutex_unlock(&la->mutex);
        return NULL;
    }

    while (1) {
        size_t want = READ_SIZE;
        if (is_wav && data_remaining >= 0) {
            if (data_remaining == 0) break;
            if (want > (size_t)data_remaining) want = (size_t)data_remaining;
        }
        size_t n = fread(buf, 1, want, stdin);
        if (n == 0) break;
        if (is_wav && data_remaining >= 0) data_remaining -= (int)n;
        live_audio_convert_and_append(la, buf, n);
    }

    free(buf);
    pthread_mutex_lock(&la->mutex);
    la->eof = 1;
    pthread_cond_signal(&la->cond);
    pthread_mutex_unlock(&la->mutex);
    return NULL;
}

qwen_live_audio_t *qwen_live_audio_start_stdin(void) {
    /* Read enough to detect WAV vs raw: we need at least 12 bytes for RIFF+WAVE,
     * but a full WAV header is typically 44 bytes. Read up to 4096 to cover
     * any extended header chunks before the data chunk. */
    uint8_t header[4096];
    size_t hdr_read = fread(header, 1, sizeof(header), stdin);
    if (hdr_read < 4) {
        fprintf(stderr, "qwen_live_audio_start_stdin: no data on stdin\n");
        return NULL;
    }

    int is_wav = 0;
    int wav_sample_rate = 0;
    int wav_channels = 0;
    int wav_bits = 0;
    int wav_format = 0;
    int data_chunk_size = -1;
    size_t data_chunk_offset = 0; /* offset into header[] where PCM data starts */
    size_t pcm_in_header = 0;     /* how many PCM bytes are in the header buffer */

    if (hdr_read >= 44 && memcmp(header, "RIFF", 4) == 0 && memcmp(header + 8, "WAVE", 4) == 0) {
        is_wav = 1;
        /* Parse WAV chunks */
        const uint8_t *p = header + 12;
        const uint8_t *end = header + hdr_read;
        while (p + 8 <= end) {
            uint32_t chunk_size = read_u32(p + 4);
            if (memcmp(p, "fmt ", 4) == 0 && chunk_size >= 16) {
                wav_format = read_u16(p + 8);
                wav_channels = read_u16(p + 10);
                wav_sample_rate = read_u32(p + 12);
                wav_bits = read_u16(p + 22);
            } else if (memcmp(p, "data", 4) == 0) {
                data_chunk_size = (int)chunk_size;
                data_chunk_offset = (size_t)(p + 8 - header);
                pcm_in_header = hdr_read - data_chunk_offset;
                if (pcm_in_header > (size_t)data_chunk_size)
                    pcm_in_header = (size_t)data_chunk_size;
                break; /* data chunk found, start streaming */
            }
            p += 8 + chunk_size;
            if (chunk_size & 1) p++;
        }

        if (wav_format != 1 || wav_bits != 16 || wav_channels < 1) {
            fprintf(stderr, "qwen_live_audio_start_stdin: unsupported WAV format "
                    "(need 16-bit PCM, got fmt=%d bits=%d)\n", wav_format, wav_bits);
            return NULL;
        }
        if (wav_sample_rate != SAMPLE_RATE) {
            fprintf(stderr, "qwen_live_audio_start_stdin: WAV sample rate is %d Hz, "
                    "but live streaming requires 16000 Hz.\n"
                    "  Hint: pipe through ffmpeg first:\n"
                    "    ... | ffmpeg -i pipe:0 -ar 16000 -ac 1 -f s16le pipe:1 | "
                    "./qwen_asr --stdin --stream\n", wav_sample_rate);
            return NULL;
        }
        if (wav_channels != 1) {
            fprintf(stderr, "qwen_live_audio_start_stdin: WAV has %d channels, "
                    "but live streaming requires mono.\n"
                    "  Hint: pipe through ffmpeg first:\n"
                    "    ... | ffmpeg -i pipe:0 -ar 16000 -ac 1 -f s16le pipe:1 | "
                    "./qwen_asr --stdin --stream\n", wav_channels);
            return NULL;
        }
        if (data_chunk_offset == 0) {
            fprintf(stderr, "qwen_live_audio_start_stdin: WAV data chunk not found in header\n");
            return NULL;
        }
        if (qwen_verbose >= 2)
            fprintf(stderr, "Live stdin: WAV detected (%d Hz, %d-bit, %d ch, data=%d bytes)\n",
                    wav_sample_rate, wav_bits, wav_channels, data_chunk_size);
    } else {
        if (qwen_verbose >= 2)
            fprintf(stderr, "Live stdin: treating as raw s16le 16kHz mono\n");
    }

    /* Allocate live audio context */
    qwen_live_audio_t *la = (qwen_live_audio_t *)calloc(1, sizeof(qwen_live_audio_t));
    if (!la) return NULL;
    pthread_mutex_init(&la->mutex, NULL);
    pthread_cond_init(&la->cond, NULL);

    /* Convert and append any PCM data already read in the header buffer */
    if (is_wav && pcm_in_header > 0) {
        live_audio_convert_and_append(la, header + data_chunk_offset, pcm_in_header);
    } else if (!is_wav) {
        /* Raw: everything we read is PCM data */
        live_audio_convert_and_append(la, header, hdr_read);
    }

    /* Spawn reader thread */
    live_reader_ctx_t *rctx = (live_reader_ctx_t *)malloc(sizeof(live_reader_ctx_t));
    if (!rctx) {
        qwen_live_audio_free(la);
        return NULL;
    }
    rctx->la = la;
    rctx->is_wav = is_wav;
    rctx->data_remaining = is_wav ? (data_chunk_size - (int)pcm_in_header) : -1;

    if (pthread_create(&la->thread, NULL, live_reader_thread, rctx) != 0) {
        fprintf(stderr, "qwen_live_audio_start_stdin: failed to create reader thread\n");
        free(rctx);
        qwen_live_audio_free(la);
        return NULL;
    }

    return la;
}

void qwen_live_audio_free(qwen_live_audio_t *la) {
    if (!la) return;
    /* If thread was started, wait for it to finish */
    if (la->thread) {
        pthread_join(la->thread, NULL);
    }
    pthread_mutex_destroy(&la->mutex);
    pthread_cond_destroy(&la->cond);
    free(la->samples);
    free(la);
}
