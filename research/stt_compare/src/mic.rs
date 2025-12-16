// std
use std::sync::mpsc::{self, Receiver, SyncSender};
// crates.io
use cpal::{
	Device, FromSample, Sample, SampleFormat, SizedSample, Stream, SupportedStreamConfig,
	traits::{DeviceTrait, HostTrait, StreamTrait},
};
// self
use crate::{config::MIC_CHUNK_QUEUE_CAPACITY, prelude::*};

pub struct MicStream {
	pub sample_rate_hz: u32,
	pub channels: usize,
	pub device_name: String,
	pub sample_format: SampleFormat,
	pub receiver: Receiver<Vec<f32>>,
	_stream: Stream,
}
impl MicStream {
	pub fn open_default() -> Result<Self> {
		let host = cpal::default_host();
		let device = host
			.default_input_device()
			.ok_or_else(|| eyre::eyre!("No default microphone input device is available."))?;
		let device_name = device.name().unwrap_or_else(|_| "Unknown microphone".to_string());
		let config = device.default_input_config().map_err(|err| {
			eyre::eyre!("Failed to load the default microphone configuration: {err}.")
		})?;
		let sample_rate_hz = config.sample_rate().0;

		if sample_rate_hz == 0 {
			return Err(eyre::eyre!("Microphone sample rate must be greater than zero."));
		}

		let channels = config.channels() as usize;
		let sample_format = config.sample_format();
		let (sender, receiver) = mpsc::sync_channel::<Vec<f32>>(MIC_CHUNK_QUEUE_CAPACITY);
		let stream = match sample_format {
			SampleFormat::I8 => build_stream::<i8>(&device, &config, channels, sender),
			SampleFormat::I16 => build_stream::<i16>(&device, &config, channels, sender),
			SampleFormat::I32 => build_stream::<i32>(&device, &config, channels, sender),
			SampleFormat::F32 => build_stream::<f32>(&device, &config, channels, sender),
			other => Err(eyre::eyre!("Unsupported microphone sample format: {other:?}.")),
		}?;

		stream.play().map_err(|e| {
			eyre::eyre!(
				"Failed to start microphone capture. If you denied permission, enable it in System Settings > Privacy & Security > Microphone. Underlying error: {e}."
			)
		})?;

		Ok(Self { sample_rate_hz, channels, device_name, sample_format, receiver, _stream: stream })
	}
}

fn build_stream<S>(
	device: &Device,
	config: &SupportedStreamConfig,
	channels: usize,
	sender: SyncSender<Vec<f32>>,
) -> Result<Stream>
where
	S: Sample + SizedSample,
	f32: FromSample<S>,
{
	device
		.build_input_stream::<S, _, _>(
			&config.config(),
			move |data: &[S], _| {
				let mut chunk = Vec::with_capacity(data.len().saturating_div(channels.max(1)));

				if channels <= 1 {
					chunk.extend(
						data.iter().map(|&sample| f32::from_sample(sample).clamp(-1.0, 1.0)),
					);
				} else {
					for frame in data.chunks_exact(channels) {
						let mut sum = 0.0f32;
						for &sample in frame {
							sum += f32::from_sample(sample);
						}
						chunk.push((sum / channels as f32).clamp(-1.0, 1.0));
					}
				}

				let _ = sender.try_send(chunk);
			},
			|err| {
				eprintln!("A microphone stream error occurred: {err}.");
			},
			None,
		)
		.map_err(|e| eyre::eyre!("Failed to build the microphone capture stream: {e}."))
}
