// self
use crate::prelude::*;

pub const REQUIRED_SAMPLE_RATE_HZ: u32 = 16_000;
pub const MIC_CHUNK_QUEUE_CAPACITY: usize = 512;

#[derive(Clone, Debug)]
pub struct RunConfig {
	pub wav_path: PathBuf,
	pub reference_text: Option<String>,
}

pub struct CommonConfig {
	pub sherpa_chunk_ms: u32,
	pub print_partials: bool,
	pub whisper_tick_every: u32,
	pub max_text_len: usize,
	pub sherpa: SherpaConfig,
	pub whisper: WhisperConfig,
}

#[derive(Clone, Debug)]
pub struct SherpaConfig {
	pub model_path: PathBuf,
	pub provider: String,
	pub num_threads: i32,
	pub decoding_method: String,
	pub max_active_paths: i32,
	pub prefer_int8: bool,
	pub use_int8_decoder: bool,
}

#[derive(Clone, Debug)]
pub struct WhisperConfig {
	pub model_path: PathBuf,
	pub num_threads: Option<i32>,
	pub language: String,
	pub force_gpu: Option<bool>,
	pub window_ms: u32,
	pub step_ms: u32,
	pub best_of: i32,
	pub beam_size: Option<i32>,
	pub beam_patience: f32,
}
