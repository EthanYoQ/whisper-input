/*
 * qwen_asr_kernels_neon.c - ARM NEON hot kernels
 */

#include "qwen_asr_kernels_impl.h"

#ifdef __ARM_NEON

#include <arm_neon.h>
#include <string.h>

void qwen_bf16_matvec_fused_neon(float *y, const float *x, const uint16_t *W_bf16,
                                 const float *bias, int in_dim, int out_dim) {
    int o = 0;

    /* Process 2 output rows at a time, 32 elements/iter, 8 accumulators */
    for (; o + 1 < out_dim; o += 2) {
        const uint16_t *w0 = W_bf16 + (size_t)o * in_dim;
        const uint16_t *w1 = W_bf16 + (size_t)(o + 1) * in_dim;
        float s0 = bias ? bias[o] : 0.0f;
        float s1 = bias ? bias[o + 1] : 0.0f;

        float32x4_t a0 = vdupq_n_f32(0.0f), a1 = vdupq_n_f32(0.0f);
        float32x4_t a2 = vdupq_n_f32(0.0f), a3 = vdupq_n_f32(0.0f);
        float32x4_t b0 = vdupq_n_f32(0.0f), b1 = vdupq_n_f32(0.0f);
        float32x4_t b2 = vdupq_n_f32(0.0f), b3 = vdupq_n_f32(0.0f);
        int k = 0;

        for (; k + 32 <= in_dim; k += 32) {
            float32x4_t x0 = vld1q_f32(x + k);
            float32x4_t x1 = vld1q_f32(x + k + 4);
            float32x4_t x2 = vld1q_f32(x + k + 8);
            float32x4_t x3 = vld1q_f32(x + k + 12);
            float32x4_t x4 = vld1q_f32(x + k + 16);
            float32x4_t x5 = vld1q_f32(x + k + 20);
            float32x4_t x6 = vld1q_f32(x + k + 24);
            float32x4_t x7 = vld1q_f32(x + k + 28);

            uint16x8_t r0a = vld1q_u16(w0 + k);
            uint16x8_t r0b = vld1q_u16(w0 + k + 8);
            uint16x8_t r0c = vld1q_u16(w0 + k + 16);
            uint16x8_t r0d = vld1q_u16(w0 + k + 24);
            a0 = vfmaq_f32(a0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0a), 16)), x0);
            a1 = vfmaq_f32(a1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0a), 16)), x1);
            a2 = vfmaq_f32(a2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0b), 16)), x2);
            a3 = vfmaq_f32(a3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0b), 16)), x3);
            a0 = vfmaq_f32(a0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0c), 16)), x4);
            a1 = vfmaq_f32(a1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0c), 16)), x5);
            a2 = vfmaq_f32(a2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0d), 16)), x6);
            a3 = vfmaq_f32(a3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0d), 16)), x7);

            uint16x8_t r1a = vld1q_u16(w1 + k);
            uint16x8_t r1b = vld1q_u16(w1 + k + 8);
            uint16x8_t r1c = vld1q_u16(w1 + k + 16);
            uint16x8_t r1d = vld1q_u16(w1 + k + 24);
            b0 = vfmaq_f32(b0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1a), 16)), x0);
            b1 = vfmaq_f32(b1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1a), 16)), x1);
            b2 = vfmaq_f32(b2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1b), 16)), x2);
            b3 = vfmaq_f32(b3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1b), 16)), x3);
            b0 = vfmaq_f32(b0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1c), 16)), x4);
            b1 = vfmaq_f32(b1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1c), 16)), x5);
            b2 = vfmaq_f32(b2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1d), 16)), x6);
            b3 = vfmaq_f32(b3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1d), 16)), x7);
        }
        for (; k + 8 <= in_dim; k += 8) {
            float32x4_t x0 = vld1q_f32(x + k);
            float32x4_t x1 = vld1q_f32(x + k + 4);
            uint16x8_t r0 = vld1q_u16(w0 + k);
            uint16x8_t r1 = vld1q_u16(w1 + k);
            a0 = vfmaq_f32(a0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0), 16)), x0);
            a1 = vfmaq_f32(a1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0), 16)), x1);
            b0 = vfmaq_f32(b0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1), 16)), x0);
            b1 = vfmaq_f32(b1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1), 16)), x1);
        }
        s0 += vaddvq_f32(vaddq_f32(vaddq_f32(a0, a2), vaddq_f32(a1, a3)));
        s1 += vaddvq_f32(vaddq_f32(vaddq_f32(b0, b2), vaddq_f32(b1, b3)));

        for (; k < in_dim; k++) {
            uint32_t bits0 = ((uint32_t)w0[k]) << 16;
            uint32_t bits1 = ((uint32_t)w1[k]) << 16;
            float wv0, wv1;
            memcpy(&wv0, &bits0, sizeof(float));
            memcpy(&wv1, &bits1, sizeof(float));
            s0 += wv0 * x[k];
            s1 += wv1 * x[k];
        }
        y[o] = s0;
        y[o + 1] = s1;
    }

    /* Handle remaining odd row */
    for (; o < out_dim; o++) {
        const uint16_t *w_row = W_bf16 + (size_t)o * in_dim;
        float sum = bias ? bias[o] : 0.0f;
        int k = 0;

        float32x4_t acc0 = vdupq_n_f32(0.0f);
        float32x4_t acc1 = vdupq_n_f32(0.0f);
        for (; k + 8 <= in_dim; k += 8) {
            uint16x8_t bf = vld1q_u16(w_row + k);
            acc0 = vfmaq_f32(acc0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(bf), 16)),
                             vld1q_f32(x + k));
            acc1 = vfmaq_f32(acc1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(bf), 16)),
                             vld1q_f32(x + k + 4));
        }
        sum += vaddvq_f32(vaddq_f32(acc0, acc1));

        for (; k < in_dim; k++) {
            uint32_t f32_bits = ((uint32_t)w_row[k]) << 16;
            float w_val;
            memcpy(&w_val, &f32_bits, sizeof(float));
            sum += w_val * x[k];
        }
        y[o] = sum;
    }
}

void qwen_argmax_bf16_range_neon(const float *x, const uint16_t *W_bf16,
                                 int in_dim, int start, int end,
                                 int *best_out, float *best_val_out) {
    int best = start;
    float best_val = -1e30f;
    int o = start;

    /* Process 2 rows at a time, 32 elements/iter, 8 accumulators per row */
    for (; o + 1 < end; o += 2) {
        const uint16_t *w0 = W_bf16 + (size_t)o * in_dim;
        const uint16_t *w1 = W_bf16 + (size_t)(o + 1) * in_dim;
        float32x4_t a0 = vdupq_n_f32(0.0f), a1 = vdupq_n_f32(0.0f);
        float32x4_t a2 = vdupq_n_f32(0.0f), a3 = vdupq_n_f32(0.0f);
        float32x4_t b0 = vdupq_n_f32(0.0f), b1 = vdupq_n_f32(0.0f);
        float32x4_t b2 = vdupq_n_f32(0.0f), b3 = vdupq_n_f32(0.0f);
        int k = 0;

        for (; k + 32 <= in_dim; k += 32) {
            float32x4_t x0 = vld1q_f32(x + k);
            float32x4_t x1 = vld1q_f32(x + k + 4);
            float32x4_t x2 = vld1q_f32(x + k + 8);
            float32x4_t x3 = vld1q_f32(x + k + 12);
            float32x4_t x4 = vld1q_f32(x + k + 16);
            float32x4_t x5 = vld1q_f32(x + k + 20);
            float32x4_t x6 = vld1q_f32(x + k + 24);
            float32x4_t x7 = vld1q_f32(x + k + 28);

            uint16x8_t r0a = vld1q_u16(w0 + k);
            uint16x8_t r0b = vld1q_u16(w0 + k + 8);
            uint16x8_t r0c = vld1q_u16(w0 + k + 16);
            uint16x8_t r0d = vld1q_u16(w0 + k + 24);
            a0 = vfmaq_f32(a0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0a), 16)), x0);
            a1 = vfmaq_f32(a1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0a), 16)), x1);
            a2 = vfmaq_f32(a2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0b), 16)), x2);
            a3 = vfmaq_f32(a3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0b), 16)), x3);
            a0 = vfmaq_f32(a0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0c), 16)), x4);
            a1 = vfmaq_f32(a1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0c), 16)), x5);
            a2 = vfmaq_f32(a2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r0d), 16)), x6);
            a3 = vfmaq_f32(a3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r0d), 16)), x7);

            uint16x8_t r1a = vld1q_u16(w1 + k);
            uint16x8_t r1b = vld1q_u16(w1 + k + 8);
            uint16x8_t r1c = vld1q_u16(w1 + k + 16);
            uint16x8_t r1d = vld1q_u16(w1 + k + 24);
            b0 = vfmaq_f32(b0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1a), 16)), x0);
            b1 = vfmaq_f32(b1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1a), 16)), x1);
            b2 = vfmaq_f32(b2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1b), 16)), x2);
            b3 = vfmaq_f32(b3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1b), 16)), x3);
            b0 = vfmaq_f32(b0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1c), 16)), x4);
            b1 = vfmaq_f32(b1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1c), 16)), x5);
            b2 = vfmaq_f32(b2, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(r1d), 16)), x6);
            b3 = vfmaq_f32(b3, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(r1d), 16)), x7);
        }

        float s0 = vaddvq_f32(vaddq_f32(vaddq_f32(a0, a2), vaddq_f32(a1, a3)));
        float s1 = vaddvq_f32(vaddq_f32(vaddq_f32(b0, b2), vaddq_f32(b1, b3)));

        for (; k < in_dim; k++) {
            uint32_t bits0 = ((uint32_t)w0[k]) << 16;
            uint32_t bits1 = ((uint32_t)w1[k]) << 16;
            float wv0, wv1;
            memcpy(&wv0, &bits0, sizeof(float));
            memcpy(&wv1, &bits1, sizeof(float));
            s0 += wv0 * x[k];
            s1 += wv1 * x[k];
        }

        if (s0 > best_val) { best_val = s0; best = o; }
        if (s1 > best_val) { best_val = s1; best = o + 1; }
    }

    for (; o < end; o++) {
        const uint16_t *w_row = W_bf16 + (size_t)o * in_dim;
        float sum = 0.0f;
        int k = 0;

        float32x4_t acc0 = vdupq_n_f32(0.0f), acc1 = vdupq_n_f32(0.0f);
        for (; k + 8 <= in_dim; k += 8) {
            uint16x8_t bf = vld1q_u16(w_row + k);
            acc0 = vfmaq_f32(acc0, vreinterpretq_f32_u32(vshll_n_u16(vget_low_u16(bf), 16)),
                             vld1q_f32(x + k));
            acc1 = vfmaq_f32(acc1, vreinterpretq_f32_u32(vshll_n_u16(vget_high_u16(bf), 16)),
                             vld1q_f32(x + k + 4));
        }
        sum += vaddvq_f32(vaddq_f32(acc0, acc1));

        for (; k < in_dim; k++) {
            uint32_t f32_bits = ((uint32_t)w_row[k]) << 16;
            float w_val;
            memcpy(&w_val, &f32_bits, sizeof(float));
            sum += w_val * x[k];
        }
        if (sum > best_val) { best_val = sum; best = o; }
    }

    *best_out = best;
    *best_val_out = best_val;
}

float qwen_dot_f32_neon(const float *a, const float *b, int n) {
    int i = 0;
    float32x4_t acc0 = vdupq_n_f32(0.0f);
    float32x4_t acc1 = vdupq_n_f32(0.0f);
    for (; i + 8 <= n; i += 8) {
        float32x4_t a0 = vld1q_f32(a + i);
        float32x4_t b0 = vld1q_f32(b + i);
        float32x4_t a1 = vld1q_f32(a + i + 4);
        float32x4_t b1 = vld1q_f32(b + i + 4);
        acc0 = vfmaq_f32(acc0, a0, b0);
        acc1 = vfmaq_f32(acc1, a1, b1);
    }
    float sum = vaddvq_f32(vaddq_f32(acc0, acc1));
    for (; i < n; i++) sum += a[i] * b[i];
    return sum;
}

void qwen_vec_scale_inplace_neon(float *dst, float scale, int n) {
    int i = 0;
    float32x4_t s = vdupq_n_f32(scale);
    for (; i + 8 <= n; i += 8) {
        float32x4_t d0 = vld1q_f32(dst + i);
        float32x4_t d1 = vld1q_f32(dst + i + 4);
        vst1q_f32(dst + i, vfmaq_f32(vdupq_n_f32(0.0f), d0, s));
        vst1q_f32(dst + i + 4, vfmaq_f32(vdupq_n_f32(0.0f), d1, s));
    }
    for (; i < n; i++) dst[i] *= scale;
}

void qwen_vec_axpy_inplace_neon(float *dst, const float *src, float alpha, int n) {
    int i = 0;
    float32x4_t a = vdupq_n_f32(alpha);
    for (; i + 8 <= n; i += 8) {
        float32x4_t d0 = vld1q_f32(dst + i);
        float32x4_t s0 = vld1q_f32(src + i);
        float32x4_t d1 = vld1q_f32(dst + i + 4);
        float32x4_t s1 = vld1q_f32(src + i + 4);
        vst1q_f32(dst + i, vfmaq_f32(d0, s0, a));
        vst1q_f32(dst + i + 4, vfmaq_f32(d1, s1, a));
    }
    for (; i < n; i++) dst[i] += alpha * src[i];
}

void qwen_vec_scale_add_neon(float *dst, const float *src, float correction, int n) {
    int i = 0;
    float32x4_t c = vdupq_n_f32(correction);
    for (; i + 8 <= n; i += 8) {
        float32x4_t d0 = vld1q_f32(dst + i);
        float32x4_t s0 = vld1q_f32(src + i);
        float32x4_t d1 = vld1q_f32(dst + i + 4);
        float32x4_t s1 = vld1q_f32(src + i + 4);
        vst1q_f32(dst + i, vfmaq_f32(s0, d0, c));
        vst1q_f32(dst + i + 4, vfmaq_f32(s1, d1, c));
    }
    for (; i < n; i++) dst[i] = dst[i] * correction + src[i];
}

#endif /* __ARM_NEON */
