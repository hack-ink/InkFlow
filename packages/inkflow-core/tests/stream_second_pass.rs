#[cfg(target_os = "macos")]
mod tests {
	use std::{
		path::PathBuf,
		time::{Duration, Instant},
	};

	use hound::WavReader;
	use inkflow_core::{AsrUpdate, InkFlowEngine, SttSettings};
	use tracing_subscriber::EnvFilter;

	fn sample_wav_path() -> PathBuf {
		PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.join("..")
			.join("..")
			.join("assets")
			.join("sample")
			.join("01_and_so_my_fellow_americans.wav")
	}

	fn load_wav_mono_float32(path: &PathBuf) -> (Vec<f32>, u32) {
		let reader = WavReader::open(path).expect("WAV file should open");
		let spec = reader.spec();
		assert_eq!(spec.channels, 1, "Test WAV must be mono");
		let sample_rate_hz = spec.sample_rate;
		let samples = reader
			.into_samples::<i16>()
			.map(|sample| sample.expect("WAV sample should decode") as f32 / i16::MAX as f32)
			.collect::<Vec<_>>();
		(samples, sample_rate_hz)
	}

	fn contains_lowercase(text: &str) -> bool {
		text.chars().any(|c| c.is_ascii_lowercase())
	}

	#[test]
	#[ignore]
	fn second_pass_produces_non_uppercase_text() {
		let _ = tracing_subscriber::fmt()
			.with_env_filter(EnvFilter::new("inkflow_core=info"))
			.with_test_writer()
			.try_init();

		let wav_path = sample_wav_path();
		assert!(wav_path.is_file(), "Sample WAV not found: {}", wav_path.display());

		let (mut samples, sample_rate_hz) = load_wav_mono_float32(&wav_path);
		assert!(!samples.is_empty(), "Sample WAV must contain audio samples");
		let trailing_silence = vec![0.0_f32; (sample_rate_hz as usize) * 2];
		samples.extend_from_slice(&trailing_silence);

		let engine = InkFlowEngine::start(SttSettings::default()).expect("Engine should start");
		engine.submit_audio(&samples, sample_rate_hz).expect("Audio submission should succeed");

		let deadline = Instant::now() + Duration::from_secs(30);
		let mut second_pass_text = None;
		let mut saw_segment_end = false;

		while Instant::now() < deadline {
			match engine.poll_update().expect("poll_update should succeed") {
				Some(AsrUpdate::SecondPass { text, .. }) => {
					second_pass_text = Some(text);
					break;
				},
				Some(AsrUpdate::SegmentEnd { .. }) => {
					saw_segment_end = true;
				},
				Some(_) => {},
				None => std::thread::sleep(Duration::from_millis(10)),
			}
		}

		let _ = engine.stop();

		assert!(saw_segment_end, "Expected at least one segment_end update");
		let text = second_pass_text.expect("Expected a second-pass update");
		assert!(!text.trim().is_empty(), "Second-pass text must be non-empty");
		assert!(
			contains_lowercase(&text),
			"Second-pass text should include lowercase letters to avoid all-caps output"
		);
	}
}
