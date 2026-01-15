use std::{
	fs,
	io::{LineWriter, Write as _},
	path::{Path, PathBuf},
	sync::mpsc,
	time::Instant,
};

use serde::Serialize;

use crate::{audio::RecordedAudio, error::AppError, events::SttStrategy};

const DEFAULT_TRACE_DIR: &str = "tmp/stt_trace";

#[derive(Clone, Debug)]
pub struct SttTrace {
	pub dir: PathBuf,
	pub session_id: String,
	started_at: Instant,
	tx: mpsc::Sender<TraceLine>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TraceLine {
	pub t_ms: u64,
	pub session_id: String,
	pub kind: String,
	pub revision: Option<u64>,
	pub strategy: Option<SttStrategy>,
	pub text: Option<String>,
	pub details: TraceDetails,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct TraceDetails {
	pub segment_id: Option<u64>,
	pub live_has_window: Option<bool>,
	pub window_generation: Option<u64>,
	pub window_job_id_last_scheduled: Option<u64>,
	pub window_job_id_last_applied: Option<u64>,
}

impl SttTrace {
	pub fn start(session_id: &str) -> Option<Self> {
		let dir = match resolve_trace_dir() {
			Ok(Some(dir)) => dir,
			Ok(None) => return None,
			Err(err) => {
				eprintln!(
					"STT trace is disabled because trace directory setup failed: {}.",
					err.message
				);
				return None;
			},
		};

		if let Err(err) = fs::create_dir_all(&dir) {
			eprintln!(
				"STT trace is disabled because the trace directory could not be created: {err}."
			);
			return None;
		}

		let display_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
		eprintln!("STT trace enabled. Directory: {}.", display_dir.display());

		let path = dir.join(format!("stt_session_{session_id}.ndjson"));
		let file = match fs::File::create(&path) {
			Ok(file) => file,
			Err(err) => {
				eprintln!("Failed to create STT trace file: {err}.");
				return None;
			},
		};
		let (tx, rx) = mpsc::channel::<TraceLine>();

		std::thread::spawn(move || {
			let mut writer = LineWriter::new(file);

			while let Ok(line) = rx.recv() {
				match serde_json::to_string(&line) {
					Ok(json) => {
						let _ = writeln!(writer, "{json}");
					},
					Err(err) => {
						eprintln!("Failed to serialize an STT trace line: {err}.");
					},
				}
			}
		});

		Some(Self { dir, session_id: session_id.to_string(), started_at: Instant::now(), tx })
	}

	pub fn audio_path(&self) -> PathBuf {
		self.dir.join(format!("stt_session_{}.wav", self.session_id))
	}

	pub fn emit(
		&self,
		kind: &str,
		revision: Option<u64>,
		strategy: Option<SttStrategy>,
		text: Option<String>,
		details: TraceDetails,
	) {
		let elapsed = self.started_at.elapsed();
		let t_ms = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;

		let _ = self.tx.send(TraceLine {
			t_ms,
			session_id: self.session_id.clone(),
			kind: kind.to_string(),
			revision,
			strategy,
			text,
			details,
		});
	}
}

pub fn write_recorded_audio_wav(path: &Path, audio: &RecordedAudio) -> Result<(), AppError> {
	if audio.sample_rate == 0 {
		return Err(AppError::new(
			"stt_trace_invalid_audio",
			"Recorded audio has an invalid sample rate.",
		));
	}

	let spec = hound::WavSpec {
		channels: 1,
		sample_rate: audio.sample_rate,
		bits_per_sample: 16,
		sample_format: hound::SampleFormat::Int,
	};

	let mut writer = hound::WavWriter::create(path, spec).map_err(|err| {
		AppError::new("stt_trace_write_failed", format!("Failed to create WAV file: {err}."))
	})?;

	for sample in &audio.samples {
		let s = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
		writer.write_sample(s).map_err(|err| {
			AppError::new("stt_trace_write_failed", format!("Failed to write WAV sample: {err}."))
		})?;
	}

	writer.finalize().map_err(|err| {
		AppError::new("stt_trace_write_failed", format!("Failed to finalize WAV file: {err}."))
	})?;

	Ok(())
}

fn resolve_trace_dir() -> Result<Option<PathBuf>, AppError> {
	if let Ok(dir) = std::env::var("INKFLOW_STT_TRACE_DIR") {
		let dir = dir.trim();
		if !dir.is_empty() {
			return Ok(Some(PathBuf::from(dir)));
		}
	}

	if let Ok(flag) = std::env::var("INKFLOW_STT_TRACE")
		&& parse_bool(&flag).unwrap_or(false)
	{
		return Ok(Some(PathBuf::from(DEFAULT_TRACE_DIR)));
	}

	Ok(None)
}

fn parse_bool(value: &str) -> Option<bool> {
	match value.trim().to_lowercase().as_str() {
		"1" | "true" | "yes" | "y" | "on" => Some(true),
		"0" | "false" | "no" | "n" | "off" => Some(false),
		_ => None,
	}
}
