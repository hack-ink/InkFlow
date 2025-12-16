// A minimal sherpa-onnx C API surface for streaming (online) ASR.
//
// This file exists to keep bindgen generation self-contained within the repo.
// It intentionally includes only the subset of sherpa-onnx APIs used by AiR.
//
// Source reference:
// - https://raw.githubusercontent.com/k2-fsa/sherpa-onnx/master/sherpa-onnx/c-api/c-api.h

#ifndef AIR_SHERPA_ONNX_C_API_H_
#define AIR_SHERPA_ONNX_C_API_H_

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SherpaOnnxOnlineTransducerModelConfig {
  const char *encoder;
  const char *decoder;
  const char *joiner;
} SherpaOnnxOnlineTransducerModelConfig;

typedef struct SherpaOnnxOnlineParaformerModelConfig {
  const char *encoder;
  const char *decoder;
} SherpaOnnxOnlineParaformerModelConfig;

typedef struct SherpaOnnxOnlineZipformer2CtcModelConfig {
  const char *model;
} SherpaOnnxOnlineZipformer2CtcModelConfig;

typedef struct SherpaOnnxOnlineNemoCtcModelConfig {
  const char *model;
} SherpaOnnxOnlineNemoCtcModelConfig;

typedef struct SherpaOnnxOnlineToneCtcModelConfig {
  const char *model;
} SherpaOnnxOnlineToneCtcModelConfig;

typedef struct SherpaOnnxOnlineModelConfig {
  SherpaOnnxOnlineTransducerModelConfig transducer;
  SherpaOnnxOnlineParaformerModelConfig paraformer;
  SherpaOnnxOnlineZipformer2CtcModelConfig zipformer2_ctc;
  const char *tokens;
  int32_t num_threads;
  const char *provider;
  int32_t debug;
  const char *model_type;
  const char *modeling_unit;
  const char *bpe_vocab;
  const char *tokens_buf;
  int32_t tokens_buf_size;
  SherpaOnnxOnlineNemoCtcModelConfig nemo_ctc;
  SherpaOnnxOnlineToneCtcModelConfig t_one_ctc;
} SherpaOnnxOnlineModelConfig;

typedef struct SherpaOnnxFeatureConfig {
  int32_t sample_rate;
  int32_t feature_dim;
} SherpaOnnxFeatureConfig;

typedef struct SherpaOnnxOnlineCtcFstDecoderConfig {
  const char *graph;
  int32_t max_active;
} SherpaOnnxOnlineCtcFstDecoderConfig;

typedef struct SherpaOnnxHomophoneReplacerConfig {
  const char *dict_dir;
  const char *lexicon;
  const char *rule_fsts;
} SherpaOnnxHomophoneReplacerConfig;

typedef struct SherpaOnnxOnlineRecognizerConfig {
  SherpaOnnxFeatureConfig feat_config;
  SherpaOnnxOnlineModelConfig model_config;

  const char *decoding_method;
  int32_t max_active_paths;

  int32_t enable_endpoint;
  float rule1_min_trailing_silence;
  float rule2_min_trailing_silence;
  float rule3_min_utterance_length;

  const char *hotwords_file;
  float hotwords_score;

  SherpaOnnxOnlineCtcFstDecoderConfig ctc_fst_decoder_config;
  const char *rule_fsts;
  const char *rule_fars;
  float blank_penalty;

  const char *hotwords_buf;
  int32_t hotwords_buf_size;
  SherpaOnnxHomophoneReplacerConfig hr;
} SherpaOnnxOnlineRecognizerConfig;

typedef struct SherpaOnnxOnlineRecognizer SherpaOnnxOnlineRecognizer;
typedef struct SherpaOnnxOnlineStream SherpaOnnxOnlineStream;

const SherpaOnnxOnlineRecognizer *SherpaOnnxCreateOnlineRecognizer(
    const SherpaOnnxOnlineRecognizerConfig *config);
void SherpaOnnxDestroyOnlineRecognizer(const SherpaOnnxOnlineRecognizer *recognizer);

const SherpaOnnxOnlineStream *SherpaOnnxCreateOnlineStream(const SherpaOnnxOnlineRecognizer *recognizer);
const SherpaOnnxOnlineStream *SherpaOnnxCreateOnlineStreamWithHotwords(
    const SherpaOnnxOnlineRecognizer *recognizer, const char *hotwords);
void SherpaOnnxDestroyOnlineStream(const SherpaOnnxOnlineStream *stream);

void SherpaOnnxOnlineStreamAcceptWaveform(const SherpaOnnxOnlineStream *stream, int32_t sample_rate,
                                          const float *samples, int32_t n);

int32_t SherpaOnnxIsOnlineStreamReady(const SherpaOnnxOnlineRecognizer *recognizer,
                                      const SherpaOnnxOnlineStream *stream);
void SherpaOnnxDecodeOnlineStream(const SherpaOnnxOnlineRecognizer *recognizer,
                                  const SherpaOnnxOnlineStream *stream);

const char *SherpaOnnxGetOnlineStreamResultAsJson(const SherpaOnnxOnlineRecognizer *recognizer,
                                                  const SherpaOnnxOnlineStream *stream);
void SherpaOnnxDestroyOnlineStreamResultJson(const char *s);

void SherpaOnnxOnlineStreamReset(const SherpaOnnxOnlineRecognizer *recognizer,
                                 const SherpaOnnxOnlineStream *stream);

void SherpaOnnxOnlineStreamInputFinished(const SherpaOnnxOnlineStream *stream);

int32_t SherpaOnnxOnlineStreamIsEndpoint(const SherpaOnnxOnlineRecognizer *recognizer,
                                         const SherpaOnnxOnlineStream *stream);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // AIR_SHERPA_ONNX_C_API_H_

