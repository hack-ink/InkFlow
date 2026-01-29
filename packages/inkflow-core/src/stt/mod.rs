mod sherpa;
mod whisper;

pub use sherpa::resolve_sherpa_config;
pub use whisper::{
	WhisperConfig, WhisperDecodeProfile, WhisperDecodeResult, WhisperDecodedSegment,
	load_whisper_context, resample_linear_to_16k, resolve_whisper_config, transcribe,
	transcribe_segments,
};

#[derive(Clone, Debug)]
pub struct WindowJobSnapshot {
	pub engine_generation: u64,
	pub window_generation: u64,
	pub job_id: u64,
	pub window_end_16k_samples: u64,
	pub window_len_16k_samples: usize,
	pub context_len_16k_samples: usize,
}
