use std::{
	pin::Pin,
	sync::{Arc, Mutex},
	time::Duration,
};

use futures_util::Stream;
use tokio::sync::{mpsc, oneshot};

use crate::error::AppError;

const MIC_CHUNK_QUEUE_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct MicRecording {
	sample_rate: u32,
	samples: Arc<Mutex<Vec<f32>>>,
}

#[cfg(target_os = "macos")]
mod macos_audio {
	use std::{
		ffi::c_void,
		mem,
		sync::{Arc, Mutex},
	};

	use tokio::sync::mpsc;

	use crate::error::AppError;

	type OSStatus = i32;
	type UInt32 = u32;
	type Float64 = f64;
	type Boolean = u8;
	type AudioQueueRef = *mut c_void;
	type AudioQueuePropertyID = UInt32;
	type AudioFormatID = UInt32;
	type AudioFormatFlags = UInt32;

	const fn fourcc(value: &[u8; 4]) -> u32 {
		u32::from_be_bytes(*value)
	}

	const K_AUDIO_FORMAT_LINEAR_PCM: AudioFormatID = fourcc(b"lpcm");

	const K_AUDIO_FORMAT_FLAG_IS_FLOAT: AudioFormatFlags = 1 << 0;
	const K_AUDIO_FORMAT_FLAG_IS_PACKED: AudioFormatFlags = 1 << 3;

	const K_AUDIO_QUEUE_DEVICE_PROPERTY_SAMPLE_RATE: AudioQueuePropertyID = fourcc(b"aqsr");

	const PREFERRED_SAMPLE_RATE_HZ: u32 = 48_000;
	const PREFERRED_CHANNELS: UInt32 = 1;
	const BUFFER_DURATION_MS: u32 = 20;
	const BUFFER_COUNT: usize = 3;

	#[repr(C)]
	struct AudioStreamBasicDescription {
		m_sample_rate: Float64,
		m_format_id: AudioFormatID,
		m_format_flags: AudioFormatFlags,
		m_bytes_per_packet: UInt32,
		m_frames_per_packet: UInt32,
		m_bytes_per_frame: UInt32,
		m_channels_per_frame: UInt32,
		m_bits_per_channel: UInt32,
		m_reserved: UInt32,
	}

	#[repr(C)]
	struct AudioTimeStamp {
		_opaque: [u8; 0],
	}

	type AudioStreamPacketDescription = c_void;

	#[repr(C)]
	struct AudioQueueBuffer {
		m_audio_data_bytes_capacity: UInt32,
		m_audio_data: *mut c_void,
		m_audio_data_byte_size: UInt32,
		_m_user_data: *mut c_void,
		m_packet_description_capacity: UInt32,
		m_packet_descriptions: *mut AudioStreamPacketDescription,
		m_packet_description_count: UInt32,
	}

	type AudioQueueBufferRef = *mut AudioQueueBuffer;

	type AudioQueueInputCallback = Option<
		unsafe extern "C" fn(
			in_user_data: *mut c_void,
			in_aq: AudioQueueRef,
			in_buffer: AudioQueueBufferRef,
			in_start_time: *const AudioTimeStamp,
			in_number_packet_descriptions: UInt32,
			in_packet_descs: *const AudioStreamPacketDescription,
		),
	>;

	#[link(name = "AudioToolbox", kind = "framework")]
	unsafe extern "C" {
		fn AudioQueueNewInput(
			in_format: *const AudioStreamBasicDescription,
			in_callback_proc: AudioQueueInputCallback,
			in_user_data: *mut c_void,
			in_callback_run_loop: *mut c_void,
			in_callback_run_loop_mode: *const c_void,
			in_flags: UInt32,
			out_aq: *mut AudioQueueRef,
		) -> OSStatus;

		fn AudioQueueAllocateBuffer(
			in_aq: AudioQueueRef,
			in_buffer_byte_size: UInt32,
			out_buffer: *mut AudioQueueBufferRef,
		) -> OSStatus;

		fn AudioQueueEnqueueBuffer(
			in_aq: AudioQueueRef,
			in_buffer: AudioQueueBufferRef,
			in_num_packet_descs: UInt32,
			in_packet_descs: *const AudioStreamPacketDescription,
		) -> OSStatus;

		fn AudioQueueStart(in_aq: AudioQueueRef, in_start_time: *const AudioTimeStamp) -> OSStatus;
		fn AudioQueueStop(in_aq: AudioQueueRef, in_immediate: Boolean) -> OSStatus;
		fn AudioQueueDispose(in_aq: AudioQueueRef, in_immediate: Boolean) -> OSStatus;

		fn AudioQueueGetProperty(
			in_aq: AudioQueueRef,
			in_id: AudioQueuePropertyID,
			out_data: *mut c_void,
			io_data_size: *mut UInt32,
		) -> OSStatus;
	}

	pub(super) struct AudioQueueInstance {
		queue: AudioQueueRef,
	}

	impl AudioQueueInstance {
		pub(super) fn stop(&mut self) {
			unsafe {
				if self.queue.is_null() {
					return;
				}

				let _ = AudioQueueStop(self.queue, 1);
				let _ = AudioQueueDispose(self.queue, 1);
				self.queue = std::ptr::null_mut();
			}
		}
	}

	impl Drop for AudioQueueInstance {
		fn drop(&mut self) {
			self.stop();
		}
	}

	pub(super) struct CallbackState {
		chunk_sender: mpsc::Sender<Vec<f32>>,
		samples: Arc<Mutex<Vec<f32>>>,
	}

	pub(super) fn start_audio_queue_input(
		chunk_sender: mpsc::Sender<Vec<f32>>,
		samples: Arc<Mutex<Vec<f32>>>,
	) -> Result<(u32, AudioQueueInstance, Box<CallbackState>), AppError> {
		unsafe {
			let requested_format = AudioStreamBasicDescription {
				m_sample_rate: PREFERRED_SAMPLE_RATE_HZ as Float64,
				m_format_id: K_AUDIO_FORMAT_LINEAR_PCM,
				m_format_flags: K_AUDIO_FORMAT_FLAG_IS_FLOAT | K_AUDIO_FORMAT_FLAG_IS_PACKED,
				m_bytes_per_packet: 4 * PREFERRED_CHANNELS,
				m_frames_per_packet: 1,
				m_bytes_per_frame: 4 * PREFERRED_CHANNELS,
				m_channels_per_frame: PREFERRED_CHANNELS,
				m_bits_per_channel: 32,
				m_reserved: 0,
			};

			let mut callback_state = Box::new(CallbackState { chunk_sender, samples });

			let mut queue: AudioQueueRef = std::ptr::null_mut();
			let status = AudioQueueNewInput(
				&requested_format as *const AudioStreamBasicDescription,
				Some(input_callback),
				(&mut *callback_state) as *mut CallbackState as *mut c_void,
				std::ptr::null_mut(),
				std::ptr::null(),
				0,
				&mut queue as *mut AudioQueueRef,
			);
			if status != 0 || queue.is_null() {
				return Err(AppError::new(
					"microphone_start_failed",
					format!("Failed to create the input audio queue (OSStatus {status})."),
				));
			}

			let buffer_frames = (PREFERRED_SAMPLE_RATE_HZ.saturating_mul(BUFFER_DURATION_MS) / 1000)
				.max(256) as usize;
			let bytes_per_frame = requested_format.m_bytes_per_frame as usize;
			let buffer_byte_size = buffer_frames
				.saturating_mul(bytes_per_frame.max(1))
				.max(mem::size_of::<f32>()) as UInt32;

			for _ in 0..BUFFER_COUNT {
				let mut buffer: AudioQueueBufferRef = std::ptr::null_mut();
				let status =
					AudioQueueAllocateBuffer(queue, buffer_byte_size, &mut buffer as *mut _);
				if status != 0 || buffer.is_null() {
					let mut instance = AudioQueueInstance { queue };
					instance.stop();
					return Err(AppError::new(
						"microphone_start_failed",
						format!("Failed to allocate an input buffer (OSStatus {status})."),
					));
				}

				(*buffer).m_audio_data_byte_size = (*buffer).m_audio_data_bytes_capacity;
				let status = AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null());
				if status != 0 {
					let mut instance = AudioQueueInstance { queue };
					instance.stop();
					return Err(AppError::new(
						"microphone_start_failed",
						format!("Failed to enqueue an input buffer (OSStatus {status})."),
					));
				}
			}

			let status = AudioQueueStart(queue, std::ptr::null());
			if status != 0 {
				let mut instance = AudioQueueInstance { queue };
				instance.stop();
				return Err(AppError::new(
					"microphone_start_failed",
					format!("Failed to start the input audio queue (OSStatus {status})."),
				));
			}

			let sample_rate = match query_sample_rate(queue) {
				Ok(rate) => rate,
				Err(err) => {
					log::warn!(
						"Falling back to the preferred sample rate because querying the audio queue sample rate failed: {}.",
						err.message
					);
					PREFERRED_SAMPLE_RATE_HZ
				},
			};

			log::info!(
				"Starting macOS microphone capture via AudioQueue input. System processing depends on the input device and macOS settings."
			);

			Ok((sample_rate, AudioQueueInstance { queue }, callback_state))
		}
	}

	unsafe fn query_sample_rate(queue: AudioQueueRef) -> Result<u32, AppError> {
		let mut value: Float64 = 0.0;
		let mut size = mem::size_of_val(&value) as UInt32;
		let status = unsafe {
			AudioQueueGetProperty(
				queue,
				K_AUDIO_QUEUE_DEVICE_PROPERTY_SAMPLE_RATE,
				&mut value as *mut _ as *mut c_void,
				&mut size as *mut UInt32,
			)
		};
		if status != 0 {
			return Err(AppError::new(
				"microphone_start_failed",
				format!("Failed to query audio queue sample rate (OSStatus {status})."),
			));
		}

		let rate = value.round();
		if rate <= 0.0 {
			return Err(AppError::new(
				"microphone_start_failed",
				"Microphone sample rate must be greater than zero.",
			));
		}

		Ok(rate.min(Float64::from(u32::MAX)) as u32)
	}

	unsafe extern "C" fn input_callback(
		in_user_data: *mut c_void,
		in_aq: AudioQueueRef,
		in_buffer: AudioQueueBufferRef,
		_in_start_time: *const AudioTimeStamp,
		_in_number_packet_descriptions: UInt32,
		_in_packet_descs: *const AudioStreamPacketDescription,
	) {
		if in_user_data.is_null() || in_aq.is_null() || in_buffer.is_null() {
			return;
		}

		let state = unsafe { &mut *(in_user_data as *mut CallbackState) };
		let buffer = unsafe { &mut *in_buffer };

		let byte_size = buffer.m_audio_data_byte_size as usize;
		if buffer.m_audio_data.is_null() || byte_size == 0 {
			buffer.m_audio_data_byte_size = buffer.m_audio_data_bytes_capacity;
			let _ = unsafe { AudioQueueEnqueueBuffer(in_aq, in_buffer, 0, std::ptr::null()) };
			return;
		}

		let sample_count = byte_size.saturating_div(mem::size_of::<f32>());
		if sample_count == 0 {
			buffer.m_audio_data_byte_size = buffer.m_audio_data_bytes_capacity;
			let _ = unsafe { AudioQueueEnqueueBuffer(in_aq, in_buffer, 0, std::ptr::null()) };
			return;
		}

		let input =
			unsafe { std::slice::from_raw_parts(buffer.m_audio_data as *const f32, sample_count) };

		let mut chunk = Vec::with_capacity(sample_count);
		chunk.extend_from_slice(input);

		for sample in &mut chunk {
			*sample = sample.clamp(-1.0, 1.0);
		}

		if let Ok(mut guard) = state.samples.lock() {
			guard.extend_from_slice(&chunk);
		}

		let _ = state.chunk_sender.try_send(chunk);

		buffer.m_audio_data_byte_size = buffer.m_audio_data_bytes_capacity;
		let _ = unsafe { AudioQueueEnqueueBuffer(in_aq, in_buffer, 0, std::ptr::null()) };
	}
}

impl MicRecording {
	pub fn take(&self) -> RecordedAudio {
		let samples =
			self.samples.lock().map(|mut guard| std::mem::take(&mut *guard)).unwrap_or_default();

		RecordedAudio { sample_rate: self.sample_rate, samples }
	}
}

#[allow(dead_code)]
pub struct RecordedAudio {
	pub sample_rate: u32,
	pub samples: Vec<f32>,
}

pub struct MicStream {
	sample_rate: u32,
	stop_tx: std::sync::mpsc::Sender<()>,
	chunk_receiver: mpsc::Receiver<Vec<f32>>,
	current_chunk: Option<Vec<f32>>,
	current_chunk_index: usize,
}

impl MicStream {
	pub async fn open_default() -> Result<(Self, MicRecording), AppError> {
		let (chunk_sender, chunk_receiver) = mpsc::channel::<Vec<f32>>(MIC_CHUNK_QUEUE_CAPACITY);
		let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
		let samples_for_callback = samples.clone();

		let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
		let (ready_tx, ready_rx) = oneshot::channel::<Result<u32, AppError>>();

		std::thread::spawn(move || {
			let start_result = start_mic_capture(chunk_sender, samples_for_callback);

			match start_result {
				Ok((sample_rate, capture)) => {
					let _ = ready_tx.send(Ok(sample_rate));
					let _ = stop_rx.recv();
					drop(capture);
				},
				Err(err) => {
					let _ = ready_tx.send(Err(err));
				},
			}
		});

		let sample_rate = match tokio::time::timeout(Duration::from_secs(5), ready_rx).await {
			Ok(Ok(Ok(sample_rate))) => sample_rate,
			Ok(Ok(Err(err))) => return Err(err),
			Ok(Err(_)) => {
				return Err(AppError::new(
					"microphone_start_failed",
					"Failed to receive microphone startup status.",
				));
			},
			Err(_) => {
				return Err(AppError::new(
					"microphone_start_timeout",
					"Timed out while waiting for microphone capture to start.",
				));
			},
		};

		let recording = MicRecording { sample_rate, samples };
		let mic = Self {
			sample_rate,
			stop_tx,
			chunk_receiver,
			current_chunk: None,
			current_chunk_index: 0,
		};

		Ok((mic, recording))
	}

	pub fn sample_rate(&self) -> u32 {
		self.sample_rate
	}

	pub async fn next_chunk(&mut self) -> Option<Vec<f32>> {
		self.chunk_receiver.recv().await
	}
}

impl Drop for MicStream {
	fn drop(&mut self) {
		let _ = self.stop_tx.send(());
	}
}

impl Stream for MicStream {
	type Item = f32;

	fn poll_next(
		mut self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Self::Item>> {
		loop {
			if let Some(chunk) = &self.current_chunk
				&& self.current_chunk_index < chunk.len()
			{
				let sample = chunk[self.current_chunk_index];
				self.current_chunk_index += 1;
				return std::task::Poll::Ready(Some(sample));
			}

			self.current_chunk = None;
			self.current_chunk_index = 0;

			match self.chunk_receiver.poll_recv(cx) {
				std::task::Poll::Ready(Some(chunk)) => {
					self.current_chunk = Some(chunk);
				},
				std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
				std::task::Poll::Pending => return std::task::Poll::Pending,
			}
		}
	}
}

#[cfg(target_os = "macos")]
fn start_mic_capture(
	chunk_sender: mpsc::Sender<Vec<f32>>,
	samples: Arc<Mutex<Vec<f32>>>,
) -> Result<(u32, MacOsMicCapture), AppError> {
	MacOsMicCapture::start(chunk_sender, samples)
}

#[cfg(not(target_os = "macos"))]
fn start_mic_capture(
	_chunk_sender: mpsc::Sender<Vec<f32>>,
	_samples: Arc<Mutex<Vec<f32>>>,
) -> Result<(u32, NoopMicCapture), AppError> {
	Err(AppError::new(
		"microphone_unsupported",
		"Microphone capture is currently only supported on macOS.",
	))
}

#[cfg(not(target_os = "macos"))]
struct NoopMicCapture;

#[cfg(target_os = "macos")]
struct MacOsMicCapture {
	audio_queue: macos_audio::AudioQueueInstance,
	_callback_state: Box<macos_audio::CallbackState>,
}

#[cfg(target_os = "macos")]
impl MacOsMicCapture {
	fn start(
		chunk_sender: mpsc::Sender<Vec<f32>>,
		samples: Arc<Mutex<Vec<f32>>>,
	) -> Result<(u32, Self), AppError> {
		let (sample_rate, audio_queue, callback_state) =
			macos_audio::start_audio_queue_input(chunk_sender, samples)?;
		Ok((sample_rate, Self { audio_queue, _callback_state: callback_state }))
	}
}

#[cfg(target_os = "macos")]
impl Drop for MacOsMicCapture {
	fn drop(&mut self) {
		self.audio_queue.stop();
	}
}
