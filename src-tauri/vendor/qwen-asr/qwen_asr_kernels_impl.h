/*
 * qwen_asr_kernels_impl.h - internal architecture dispatch for hot kernels
 */

#ifndef QWEN_ASR_KERNELS_IMPL_H
#define QWEN_ASR_KERNELS_IMPL_H

#include <stdint.h>

void qwen_bf16_matvec_fused_generic(float *y, const float *x, const uint16_t *W_bf16,
                                    const float *bias, int in_dim, int out_dim);
void qwen_argmax_bf16_range_generic(const float *x, const uint16_t *W_bf16,
                                    int in_dim, int start, int end,
                                    int *best_out, float *best_val_out);
float qwen_dot_f32_generic(const float *a, const float *b, int n);
void qwen_vec_scale_inplace_generic(float *dst, float scale, int n);
void qwen_vec_axpy_inplace_generic(float *dst, const float *src, float alpha, int n);
void qwen_vec_scale_add_generic(float *dst, const float *src, float correction, int n);

#ifdef __ARM_NEON
void qwen_bf16_matvec_fused_neon(float *y, const float *x, const uint16_t *W_bf16,
                                 const float *bias, int in_dim, int out_dim);
void qwen_argmax_bf16_range_neon(const float *x, const uint16_t *W_bf16,
                                 int in_dim, int start, int end,
                                 int *best_out, float *best_val_out);
float qwen_dot_f32_neon(const float *a, const float *b, int n);
void qwen_vec_scale_inplace_neon(float *dst, float scale, int n);
void qwen_vec_axpy_inplace_neon(float *dst, const float *src, float alpha, int n);
void qwen_vec_scale_add_neon(float *dst, const float *src, float correction, int n);

#define qwen_bf16_matvec_fused_impl qwen_bf16_matvec_fused_neon
#define qwen_argmax_bf16_range_impl qwen_argmax_bf16_range_neon
#define qwen_dot_f32_impl qwen_dot_f32_neon
#define qwen_vec_scale_inplace_impl qwen_vec_scale_inplace_neon
#define qwen_vec_axpy_inplace_impl qwen_vec_axpy_inplace_neon
#define qwen_vec_scale_add_impl qwen_vec_scale_add_neon

#elif defined(__AVX2__) && defined(__FMA__)
void qwen_bf16_matvec_fused_avx(float *y, const float *x, const uint16_t *W_bf16,
                                 const float *bias, int in_dim, int out_dim);
void qwen_argmax_bf16_range_avx(const float *x, const uint16_t *W_bf16,
                                 int in_dim, int start, int end,
                                 int *best_out, float *best_val_out);
float qwen_dot_f32_avx(const float *a, const float *b, int n);
void qwen_vec_scale_inplace_avx(float *dst, float scale, int n);
void qwen_vec_axpy_inplace_avx(float *dst, const float *src, float alpha, int n);
void qwen_vec_scale_add_avx(float *dst, const float *src, float correction, int n);

#define qwen_bf16_matvec_fused_impl qwen_bf16_matvec_fused_avx
#define qwen_argmax_bf16_range_impl qwen_argmax_bf16_range_avx
#define qwen_dot_f32_impl qwen_dot_f32_avx
#define qwen_vec_scale_inplace_impl qwen_vec_scale_inplace_avx
#define qwen_vec_axpy_inplace_impl qwen_vec_axpy_inplace_avx
#define qwen_vec_scale_add_impl qwen_vec_scale_add_avx

#else
#define qwen_bf16_matvec_fused_impl qwen_bf16_matvec_fused_generic
#define qwen_argmax_bf16_range_impl qwen_argmax_bf16_range_generic
#define qwen_dot_f32_impl qwen_dot_f32_generic
#define qwen_vec_scale_inplace_impl qwen_vec_scale_inplace_generic
#define qwen_vec_axpy_inplace_impl qwen_vec_axpy_inplace_generic
#define qwen_vec_scale_add_impl qwen_vec_scale_add_generic
#endif

#endif /* QWEN_ASR_KERNELS_IMPL_H */
