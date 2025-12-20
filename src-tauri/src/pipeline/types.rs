use std::sync::Arc;

use crate::{
	audio::{MicRecording, MicStream},
	settings::SttSettings,
	stt,
};

#[derive(Debug)]
pub(crate) enum AsrUpdate {
	SherpaPartial(String),
	WindowScheduled(stt::WindowJobSnapshot),
	WindowResult {
		snapshot: stt::WindowJobSnapshot,
		result: stt::WhisperDecodeResult,
	},
	SegmentEnd {
		segment_id: u64,
		sherpa_text: String,
		committed_end_16k_samples: u64,
		window_generation_after: u64,
	},
	EndpointReset {
		window_generation_after: u64,
	},
	SecondPass {
		segment_id: u64,
		text: String,
	},
}

pub(crate) struct DictationInit {
	pub(crate) stt_settings: SttSettings,
	pub(crate) recognizer: sherpa_onnx::OnlineRecognizer,
	pub(crate) stream: sherpa_onnx::OnlineStream,
	pub(crate) whisper_config: stt::WhisperConfig,
	pub(crate) whisper_ctx: Arc<whisper_rs::WhisperContext>,
	pub(crate) window_profile: stt::WhisperDecodeProfile,
	pub(crate) second_pass_profile: stt::WhisperDecodeProfile,
	pub(crate) mic: MicStream,
	pub(crate) recording: MicRecording,
	pub(crate) sample_rate: u32,
	pub(crate) engine_generation: u64,
}
