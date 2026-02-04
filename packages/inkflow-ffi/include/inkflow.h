#ifndef INKFLOW_H
#define INKFLOW_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle. Do not access fields directly.
typedef struct InkFlowHandle {
	uint8_t _private;
} InkFlowHandle;

// The callback receives a UTF-8 JSON string that is only valid for the duration of the call.
typedef void (*inkflow_update_cb)(const char *utf8, void *user_data);

enum {
	INKFLOW_OK = 0,
	INKFLOW_ERR_NULL = 1,
	INKFLOW_ERR_INVALID_ARGUMENT = 2,
	INKFLOW_ERR_INTERNAL = 3,
};

InkFlowHandle *inkflow_engine_create(void);
void inkflow_engine_destroy(InkFlowHandle *handle);

int32_t inkflow_engine_submit_audio(
	InkFlowHandle *handle,
	const float *samples,
	size_t sample_count,
	uint32_t sample_rate_hz
);

int32_t inkflow_engine_force_finalize(InkFlowHandle *handle);

int32_t inkflow_engine_register_callback(
	InkFlowHandle *handle,
	inkflow_update_cb callback,
	void *user_data
);

void inkflow_engine_unregister_callback(InkFlowHandle *handle);

#ifdef __cplusplus
}
#endif

#endif
