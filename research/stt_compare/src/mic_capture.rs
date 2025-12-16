// std
use std::{
	env,
	io::BufRead,
	process,
	sync::mpsc::{self, RecvTimeoutError, TryRecvError},
	thread,
	time::{Duration, SystemTime},
};
// crates.io
use hound::{SampleFormat, WavSpec, WavWriter};
// self
use crate::{mic::MicStream, prelude::*};

pub struct CapturedWav {
	pub wav_path: PathBuf,
}

pub fn capture_to_temp_wav() -> Result<CapturedWav> {
	let mic_stream = MicStream::open_default()?;

	println!("[config] input=mic");
	println!(
		"[config] microphone={} sample_rate_hz={} channels={} format={:?}",
		mic_stream.device_name,
		mic_stream.sample_rate_hz,
		mic_stream.channels,
		mic_stream.sample_format
	);
	println!("[info] Recording microphone input. Press Enter to stop.");
	println!();

	let (stop_sender, stop_receiver) = mpsc::channel::<()>();

	thread::spawn(move || {
		let mut buffer: String = String::new();
		let mut stdin = std::io::stdin().lock();
		let _ = stdin.read_line(&mut buffer);
		let _ = stop_sender.send(());
	});

	let mut samples: Vec<f32> = Vec::new();

	loop {
		match stop_receiver.try_recv() {
			Ok(()) | Err(TryRecvError::Disconnected) => break,
			Err(TryRecvError::Empty) => {},
		}
		match mic_stream.receiver.recv_timeout(Duration::from_millis(50)) {
			Ok(chunk) => {
				if chunk.is_empty() {
					continue;
				}

				samples.extend_from_slice(&chunk);
			},
			Err(RecvTimeoutError::Timeout) => {},
			Err(RecvTimeoutError::Disconnected) => break,
		}
	}

	let sample_rate_hz = mic_stream.sample_rate_hz;

	drop(mic_stream);

	if samples.is_empty() {
		return Err(eyre::eyre!("Microphone capture duration must be greater than zero."));
	}

	let duration_ms = crate::audio::samples_to_ms(samples.len(), sample_rate_hz).max(1);
	let wav_path = temp_wav_path("stt_compare");

	write_wav_i16_mono(&wav_path, sample_rate_hz, &samples)?;

	println!("[config] captured_wav={} duration_ms={duration_ms}", wav_path.display());
	println!();

	Ok(CapturedWav { wav_path })
}

fn temp_wav_path(prefix: &str) -> PathBuf {
	let pid = process::id();
	let timestamp_ms = SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.map(|d| d.as_millis())
		.unwrap_or(0);

	env::temp_dir().join(format!("{prefix}_{pid}_{timestamp_ms}.wav"))
}

fn write_wav_i16_mono(path: &Path, sample_rate_hz: u32, samples: &[f32]) -> Result<()> {
	let spec = WavSpec {
		channels: 1,
		sample_rate: sample_rate_hz,
		bits_per_sample: 16,
		sample_format: SampleFormat::Int,
	};
	let mut writer = WavWriter::create(path, spec)
		.wrap_err_with(|| format!("Failed to create a temporary WAV file: {}.", path.display()))?;

	for sample in samples {
		writer
			.write_sample(float_to_i16(*sample))
			.wrap_err_with(|| format!("Failed to write WAV samples: {}.", path.display()))?;
	}

	writer.finalize().wrap_err_with(|| format!("Failed to finalize WAV: {}.", path.display()))?;
	Ok(())
}

fn float_to_i16(sample: f32) -> i16 {
	let clamped = sample.clamp(-1.0, 1.0);

	(clamped * i16::MAX as f32).round() as i16
}
