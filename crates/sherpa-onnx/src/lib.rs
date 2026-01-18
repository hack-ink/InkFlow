//! Safe wrapper around the sherpa-onnx C API.
//!
//! This crate dynamically loads the sherpa-onnx C API shared library at runtime and exposes a
//! minimal subset of the streaming online recognizer API.
//!
//! The shared library lookup follows these rules:
//!
//! - If `INKFLOW_SHERPA_ONNX_DYLIB` is set, it is treated as an explicit path to the shared
//!   library.
//! - Otherwise, the loader searches for the platform-specific library name in common locations
//!   relative to the current executable.

#![deny(clippy::all, missing_docs, unused_crate_dependencies)]

// std
use std::{
	error::Error,
	ffi::{CStr, CString, c_char},
	fmt::{Display, Formatter, Result as FmtResult},
	mem,
	path::PathBuf,
	sync::Arc,
};
// crates.io
use libloading::Library;
use serde::Deserialize;
// self
use sherpa_onnx_sys::{
	SherpaOnnxOnlineModelConfig, SherpaOnnxOnlineRecognizer, SherpaOnnxOnlineRecognizerConfig,
	SherpaOnnxOnlineStream,
};

type CreateOnlineRecognizerFn = unsafe extern "C" fn(
	config: *const SherpaOnnxOnlineRecognizerConfig,
) -> *const SherpaOnnxOnlineRecognizer;
type DestroyOnlineRecognizerFn =
	unsafe extern "C" fn(recognizer: *const SherpaOnnxOnlineRecognizer);

type CreateOnlineStreamFn = unsafe extern "C" fn(
	recognizer: *const SherpaOnnxOnlineRecognizer,
) -> *const SherpaOnnxOnlineStream;
type DestroyOnlineStreamFn = unsafe extern "C" fn(stream: *const SherpaOnnxOnlineStream);

type OnlineStreamAcceptWaveformFn = unsafe extern "C" fn(
	stream: *const SherpaOnnxOnlineStream,
	sample_rate: i32,
	samples: *const f32,
	n: i32,
);

type IsOnlineStreamReadyFn = unsafe extern "C" fn(
	recognizer: *const SherpaOnnxOnlineRecognizer,
	stream: *const SherpaOnnxOnlineStream,
) -> i32;

type DecodeOnlineStreamFn = unsafe extern "C" fn(
	recognizer: *const SherpaOnnxOnlineRecognizer,
	stream: *const SherpaOnnxOnlineStream,
);

type GetOnlineStreamResultAsJsonFn = unsafe extern "C" fn(
	recognizer: *const SherpaOnnxOnlineRecognizer,
	stream: *const SherpaOnnxOnlineStream,
) -> *const c_char;

type DestroyOnlineStreamResultJsonFn = unsafe extern "C" fn(s: *const c_char);

type OnlineStreamResetFn = unsafe extern "C" fn(
	recognizer: *const SherpaOnnxOnlineRecognizer,
	stream: *const SherpaOnnxOnlineStream,
);

type OnlineStreamInputFinishedFn = unsafe extern "C" fn(stream: *const SherpaOnnxOnlineStream);

type OnlineStreamIsEndpointFn = unsafe extern "C" fn(
	recognizer: *const SherpaOnnxOnlineRecognizer,
	stream: *const SherpaOnnxOnlineStream,
) -> i32;

#[derive(Clone, Debug)]
/// Error type returned by the sherpa-onnx wrapper.
pub struct SherpaError {
	/// A human-readable error message.
	pub message: String,
}
impl Display for SherpaError {
	fn fmt(&self, f: &mut Formatter) -> FmtResult {
		f.write_str(&self.message)
	}
}
impl Error for SherpaError {}

#[derive(Clone, Debug)]
/// Configuration for creating an online streaming recognizer.
///
/// This struct mirrors the fields of the underlying sherpa-onnx C API configuration.
pub struct OnlineRecognizerConfig {
	/// Path to the model's token file.
	pub tokens: String,
	/// Path to the model's encoder file.
	pub encoder: String,
	/// Path to the model's decoder file.
	pub decoder: String,
	/// Path to the model's joiner file.
	pub joiner: String,
	/// Execution provider name (for example, `cpu` or `coreml`).
	pub provider: String,
	/// Number of threads used by the runtime.
	pub num_threads: i32,
	/// Input audio sample rate, in Hz.
	pub sample_rate: i32,
	/// Feature dimension (typically `80` for log-mel features).
	pub feature_dim: i32,
	/// Decoding method name (for example, `greedy_search` or `modified_beam_search`).
	pub decoding_method: String,
	/// Maximum number of active paths during decoding (used by some decoding methods).
	pub max_active_paths: i32,
	/// Whether endpoint detection is enabled.
	pub enable_endpoint: bool,
	/// Endpoint rule 1: minimum trailing silence, in seconds.
	pub rule1_min_trailing_silence: f32,
	/// Endpoint rule 2: minimum trailing silence, in seconds.
	pub rule2_min_trailing_silence: f32,
	/// Endpoint rule 3: minimum utterance length, in seconds.
	pub rule3_min_utterance_length: f32,
}
impl Default for OnlineRecognizerConfig {
	fn default() -> Self {
		Self {
			tokens: String::new(),
			encoder: String::new(),
			decoder: String::new(),
			joiner: String::new(),
			provider: "cpu".into(),
			num_threads: 2,
			sample_rate: 16_000,
			feature_dim: 80,
			decoding_method: "greedy_search".into(),
			max_active_paths: 4,
			enable_endpoint: true,
			rule1_min_trailing_silence: 2.4,
			rule2_min_trailing_silence: 1.2,
			rule3_min_utterance_length: 300.0,
		}
	}
}

#[derive(Clone, Debug, Deserialize)]
/// Result returned by the online recognizer.
///
/// sherpa-onnx reports recognition results as JSON; this type is deserialized from that payload.
pub struct OnlineResult {
	/// Current recognized text.
	#[serde(default)]
	pub text: String,
	/// Tokens corresponding to the current result.
	#[serde(default)]
	pub tokens: Vec<String>,
	/// Token timestamps, in seconds.
	#[serde(default)]
	pub timestamps: Vec<f32>,
	/// Segment identifier reported by sherpa-onnx.
	#[serde(default)]
	pub segment: i32,
	/// Segment start time, in seconds.
	#[serde(default)]
	pub start_time: f32,
	/// Whether this result is final.
	#[serde(default)]
	pub is_final: bool,
}

#[derive(Clone)]
/// Online streaming recognizer backed by the sherpa-onnx C API.
pub struct OnlineRecognizer {
	inner: Arc<OnlineRecognizerInner>,
}
impl OnlineRecognizer {
	/// Creates a new online recognizer with the provided configuration.
	pub fn new(config: OnlineRecognizerConfig) -> Result<Self, SherpaError> {
		let api = Arc::new(SherpaOnnxApi::load()?);
		let keepalive = RecognizerConfigKeepAlive {
			_tokens: CString::new(config.tokens)
				.map_err(|e| SherpaError { message: format!("Invalid tokens path: {e}.") })?,
			_encoder: CString::new(config.encoder)
				.map_err(|e| SherpaError { message: format!("Invalid encoder path: {e}.") })?,
			_decoder: CString::new(config.decoder)
				.map_err(|e| SherpaError { message: format!("Invalid decoder path: {e}.") })?,
			_joiner: CString::new(config.joiner)
				.map_err(|e| SherpaError { message: format!("Invalid joiner path: {e}.") })?,
			_provider: CString::new(config.provider)
				.map_err(|e| SherpaError { message: format!("Invalid provider: {e}.") })?,
			_decoding_method: CString::new(config.decoding_method)
				.map_err(|e| SherpaError { message: format!("Invalid decoding method: {e}.") })?,
		};
		let mut c_config: SherpaOnnxOnlineRecognizerConfig =
			unsafe { mem::zeroed::<SherpaOnnxOnlineRecognizerConfig>() };

		c_config.model_config = SherpaOnnxOnlineModelConfig {
			tokens: keepalive._tokens.as_ptr(),
			num_threads: config.num_threads,
			provider: keepalive._provider.as_ptr(),
			debug: 0,
			..unsafe { mem::zeroed::<SherpaOnnxOnlineModelConfig>() }
		};
		c_config.model_config.transducer.encoder = keepalive._encoder.as_ptr();
		c_config.model_config.transducer.decoder = keepalive._decoder.as_ptr();
		c_config.model_config.transducer.joiner = keepalive._joiner.as_ptr();
		c_config.decoding_method = keepalive._decoding_method.as_ptr();
		c_config.max_active_paths = config.max_active_paths;
		c_config.feat_config.sample_rate = config.sample_rate;
		c_config.feat_config.feature_dim = config.feature_dim;

		if config.enable_endpoint {
			c_config.enable_endpoint = 1;
			c_config.rule1_min_trailing_silence = config.rule1_min_trailing_silence;
			c_config.rule2_min_trailing_silence = config.rule2_min_trailing_silence;
			c_config.rule3_min_utterance_length = config.rule3_min_utterance_length;
		}

		let ptr = unsafe { (api.create_online_recognizer)(&c_config) };

		if ptr.is_null() {
			return Err(SherpaError {
				message:
					"Failed to create the sherpa-onnx online recognizer. Check model paths and runtime libraries."
						.into(),
			});
		}

		Ok(Self {
			inner: Arc::new(OnlineRecognizerInner { api, ptr, _config_keepalive: keepalive }),
		})
	}

	/// Creates a new online stream associated with this recognizer.
	pub fn create_stream(&self) -> Result<OnlineStream, SherpaError> {
		let stream_ptr = unsafe { (self.inner.api.create_online_stream)(self.inner.ptr) };

		if stream_ptr.is_null() {
			return Err(SherpaError {
				message: "Failed to create the sherpa-onnx online stream.".into(),
			});
		}

		Ok(OnlineStream { inner: self.inner.clone(), ptr: stream_ptr })
	}

	/// Runs decoder steps until the stream is no longer ready.
	pub fn decode(&self, stream: &OnlineStream) {
		unsafe {
			while (self.inner.api.is_online_stream_ready)(self.inner.ptr, stream.ptr) != 0 {
				(self.inner.api.decode_online_stream)(self.inner.ptr, stream.ptr);
			}
		}
	}

	/// Returns the current recognition result for a stream as a parsed struct.
	pub fn result_json(&self, stream: &OnlineStream) -> Result<OnlineResult, SherpaError> {
		let json_ptr = unsafe {
			(self.inner.api.get_online_stream_result_as_json)(self.inner.ptr, stream.ptr)
		};

		if json_ptr.is_null() {
			return Err(SherpaError {
				message: "sherpa-onnx returned a NULL JSON pointer.".into(),
			});
		}

		let json = unsafe { CStr::from_ptr(json_ptr) }.to_string_lossy().to_string();

		unsafe { (self.inner.api.destroy_online_stream_result_json)(json_ptr) };

		serde_json::from_str::<OnlineResult>(&json).map_err(|err| SherpaError {
			message: format!("Failed to parse sherpa-onnx JSON result: {err}."),
		})
	}
}

/// Online stream used to feed audio into the recognizer.
pub struct OnlineStream {
	inner: Arc<OnlineRecognizerInner>,
	ptr: *const SherpaOnnxOnlineStream,
}
impl OnlineStream {
	/// Feeds PCM samples into the stream.
	///
	/// `samples` must contain mono f32 samples. The C API expects values in the range `[-1.0,
	/// 1.0]`.
	pub fn accept_waveform(&self, sample_rate: i32, samples: &[f32]) {
		let Ok(n) = i32::try_from(samples.len()) else { return };

		unsafe {
			(self.inner.api.online_stream_accept_waveform)(
				self.ptr,
				sample_rate,
				samples.as_ptr(),
				n,
			);
		}
	}

	/// Signals that no more audio will be provided for this stream.
	pub fn input_finished(&self) {
		unsafe {
			(self.inner.api.online_stream_input_finished)(self.ptr);
		}
	}

	/// Returns `true` when the underlying endpoint detector fires.
	pub fn is_endpoint(&self) -> bool {
		unsafe { (self.inner.api.online_stream_is_endpoint)(self.inner.ptr, self.ptr) != 0 }
	}

	/// Resets the stream state after an endpoint is handled.
	pub fn reset(&self) {
		unsafe {
			(self.inner.api.online_stream_reset)(self.inner.ptr, self.ptr);
		}
	}
}
unsafe impl Send for OnlineStream {}
unsafe impl Sync for OnlineStream {}
impl Drop for OnlineStream {
	fn drop(&mut self) {
		unsafe {
			(self.inner.api.destroy_online_stream)(self.ptr);
		}
	}
}

struct SherpaOnnxApi {
	_lib: Library,
	_loaded_from: Option<PathBuf>,
	create_online_recognizer: CreateOnlineRecognizerFn,
	destroy_online_recognizer: DestroyOnlineRecognizerFn,
	create_online_stream: CreateOnlineStreamFn,
	destroy_online_stream: DestroyOnlineStreamFn,
	online_stream_accept_waveform: OnlineStreamAcceptWaveformFn,
	is_online_stream_ready: IsOnlineStreamReadyFn,
	decode_online_stream: DecodeOnlineStreamFn,
	get_online_stream_result_as_json: GetOnlineStreamResultAsJsonFn,
	destroy_online_stream_result_json: DestroyOnlineStreamResultJsonFn,
	online_stream_reset: OnlineStreamResetFn,
	online_stream_input_finished: OnlineStreamInputFinishedFn,
	online_stream_is_endpoint: OnlineStreamIsEndpointFn,
}
impl SherpaOnnxApi {
	fn load() -> Result<Self, SherpaError> {
		let lib_name = if cfg!(target_os = "macos") {
			"libsherpa-onnx-c-api.dylib"
		} else if cfg!(target_os = "windows") {
			"sherpa-onnx-c-api.dll"
		} else {
			"libsherpa-onnx-c-api.so"
		};
		let mut candidates = Vec::new();

		if let Ok(override_path) = std::env::var("INKFLOW_SHERPA_ONNX_DYLIB") {
			let override_path = override_path.trim();

			if !override_path.is_empty() {
				candidates.push(PathBuf::from(override_path));
			}
		}
		if let Ok(exe_path) = std::env::current_exe()
			&& let Some(exe_dir) = exe_path.parent()
		{
			// App bundle layout: <App>.app/Contents/MacOS/<exe> and dylibs in
			// Contents/Frameworks.
			if cfg!(target_os = "macos")
				&& let Some(contents_dir) = exe_dir.parent()
			{
				candidates.push(contents_dir.join("Frameworks").join(lib_name));
			}

			// Local development: dylibs adjacent to the executable.
			candidates.push(exe_dir.join(lib_name));

			// Repo development layout: <repo>/third_party/sherpa-onnx-prefix/lib/<dylib>.
			for ancestor in exe_dir.ancestors() {
				let candidate = ancestor
					.join("third_party")
					.join("sherpa-onnx-prefix")
					.join("lib")
					.join(lib_name);

				if candidate.is_file() {
					candidates.push(candidate);
					break;
				}
			}
		}

		// Fall back to the platform dynamic linker search path.
		candidates.push(PathBuf::from(lib_name));

		let mut errors: Vec<String> = Vec::new();
		let mut lib: Option<Library> = None;
		let mut loaded_from: Option<PathBuf> = None;

		for candidate in candidates {
			match unsafe { Library::new(&candidate) } {
				Ok(v) => {
					lib = Some(v);
					loaded_from = Some(candidate);

					break;
				},
				Err(e) => {
					errors.push(format!("{candidate:?}: {e}."));
				},
			}
		}

		let Some(lib) = lib else {
			let mut message = String::new();

			message.push_str("Failed to load the sherpa-onnx C API dynamic library.\n");

			if !errors.is_empty() {
				message.push_str("Tried:\n");

				for err in errors {
					message.push_str("- ");
					message.push_str(&err);
					message.push('\n');
				}
			}

			message.push_str(
				"Hint: Run `cargo make setup-macos` to build the native libraries, or set INKFLOW_SHERPA_ONNX_DYLIB to an absolute path.",
			);

			return Err(SherpaError { message });
		};

		unsafe {
			let create_online_recognizer = *lib
				.get::<CreateOnlineRecognizerFn>(b"SherpaOnnxCreateOnlineRecognizer\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxCreateOnlineRecognizer: {e}."),
				})?;
			let destroy_online_recognizer = *lib
				.get::<DestroyOnlineRecognizerFn>(b"SherpaOnnxDestroyOnlineRecognizer\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxDestroyOnlineRecognizer: {e}."),
				})?;
			let create_online_stream = *lib
				.get::<CreateOnlineStreamFn>(b"SherpaOnnxCreateOnlineStream\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxCreateOnlineStream: {e}."),
				})?;
			let destroy_online_stream = *lib
				.get::<DestroyOnlineStreamFn>(b"SherpaOnnxDestroyOnlineStream\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxDestroyOnlineStream: {e}."),
				})?;
			let online_stream_accept_waveform = *lib
				.get::<OnlineStreamAcceptWaveformFn>(b"SherpaOnnxOnlineStreamAcceptWaveform\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxOnlineStreamAcceptWaveform: {e}."),
				})?;
			let is_online_stream_ready = *lib
				.get::<IsOnlineStreamReadyFn>(b"SherpaOnnxIsOnlineStreamReady\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxIsOnlineStreamReady: {e}."),
				})?;
			let decode_online_stream = *lib
				.get::<DecodeOnlineStreamFn>(b"SherpaOnnxDecodeOnlineStream\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxDecodeOnlineStream: {e}."),
				})?;
			let get_online_stream_result_as_json = *lib
				.get::<GetOnlineStreamResultAsJsonFn>(b"SherpaOnnxGetOnlineStreamResultAsJson\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxGetOnlineStreamResultAsJson: {e}."),
				})?;
			let destroy_online_stream_result_json = *lib
				.get::<DestroyOnlineStreamResultJsonFn>(
					b"SherpaOnnxDestroyOnlineStreamResultJson\0",
				)
				.map_err(|e| SherpaError {
					message: format!(
						"Failed to load SherpaOnnxDestroyOnlineStreamResultJson: {e}.",
					),
				})?;
			let online_stream_reset = *lib
				.get::<OnlineStreamResetFn>(b"SherpaOnnxOnlineStreamReset\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxOnlineStreamReset: {e}."),
				})?;
			let online_stream_input_finished = *lib
				.get::<OnlineStreamInputFinishedFn>(b"SherpaOnnxOnlineStreamInputFinished\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxOnlineStreamInputFinished: {e}."),
				})?;
			let online_stream_is_endpoint = *lib
				.get::<OnlineStreamIsEndpointFn>(b"SherpaOnnxOnlineStreamIsEndpoint\0")
				.map_err(|e| SherpaError {
					message: format!("Failed to load SherpaOnnxOnlineStreamIsEndpoint: {e}."),
				})?;

			Ok(Self {
				_lib: lib,
				_loaded_from: loaded_from,
				create_online_recognizer,
				destroy_online_recognizer,
				create_online_stream,
				destroy_online_stream,
				online_stream_accept_waveform,
				is_online_stream_ready,
				decode_online_stream,
				get_online_stream_result_as_json,
				destroy_online_stream_result_json,
				online_stream_reset,
				online_stream_input_finished,
				online_stream_is_endpoint,
			})
		}
	}
}

struct OnlineRecognizerInner {
	api: Arc<SherpaOnnxApi>,
	ptr: *const SherpaOnnxOnlineRecognizer,
	_config_keepalive: RecognizerConfigKeepAlive,
}
impl Drop for OnlineRecognizerInner {
	fn drop(&mut self) {
		unsafe {
			(self.api.destroy_online_recognizer)(self.ptr);
		}
	}
}
unsafe impl Send for OnlineRecognizerInner {}
unsafe impl Sync for OnlineRecognizerInner {}

struct RecognizerConfigKeepAlive {
	_tokens: CString,
	_encoder: CString,
	_decoder: CString,
	_joiner: CString,
	_provider: CString,
	_decoding_method: CString,
}
