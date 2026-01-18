mod modes;
mod pipeline;
mod state;
mod worker;

use crate::{error::AppError, settings::SttSettings, stt};

use modes::ModeRouter;
use pipeline::SttPipeline;

#[derive(Debug)]
pub enum AsrUpdate {
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

pub struct InkFlowEngine {
	pipeline: SttPipeline,
}

impl InkFlowEngine {
	pub fn start(stt_settings: SttSettings) -> Result<Self, AppError> {
		let plan = ModeRouter::resolve(&stt_settings);
		let pipeline = SttPipeline::start(plan, stt_settings)?;
		Ok(Self { pipeline })
	}

	pub fn submit_audio(&self, samples: &[f32], sample_rate_hz: u32) -> Result<(), AppError> {
		self.pipeline.submit_audio(samples, sample_rate_hz)
	}

	pub fn poll_update(&self) -> Result<Option<AsrUpdate>, AppError> {
		self.pipeline.poll_update()
	}

	pub fn stop(self) -> Result<(), AppError> {
		self.pipeline.stop()
	}
}

fn mean_abs(samples: &[f32]) -> f32 {
	if samples.is_empty() {
		return 0.0;
	}

	let mut sum = 0.0_f32;
	for s in samples {
		sum += s.abs();
	}

	sum / samples.len() as f32
}
