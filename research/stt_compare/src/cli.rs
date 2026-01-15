// std
use std::thread;
// crates.io
use clap::{
	Parser, ValueEnum,
	builder::{
		Styles,
		styling::{AnsiColor, Effects},
	},
};
// self
use crate::{
	config::{CommonConfig, SherpaConfig, WhisperConfig},
	prelude::*,
	references::{self, ManifestCase},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum WhisperGpuMode {
	Auto,
	On,
	Off,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum SherpaProvider {
	Cpu,
	Coreml,
}
impl SherpaProvider {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Cpu => "cpu",
			Self::Coreml => "coreml",
		}
	}
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum SherpaDecodingMethod {
	GreedySearch,
	ModifiedBeamSearch,
}
impl SherpaDecodingMethod {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::GreedySearch => "greedy_search",
			Self::ModifiedBeamSearch => "modified_beam_search",
		}
	}
}

/// Speech-to-text A/B comparison harness CLI.
#[derive(Debug, Parser)]
#[command(
	version = concat!(
		env!("CARGO_PKG_VERSION"),
		"-",
		env!("VERGEN_GIT_SHA"),
		"-",
		env!("VERGEN_CARGO_TARGET_TRIPLE"),
	),
	rename_all = "kebab",
	styles = styles(),
)]
pub struct Cli {
	/// Use the default microphone input.
	#[arg(long, conflicts_with = "manifest")]
	pub mic: bool,
	/// Input manifest TOML containing WAV paths and optional reference text.
	/// If omitted and `research/stt_compare/manifest.toml` exists, it is used automatically.
	#[arg(long, value_name = "PATH")]
	pub manifest: Option<PathBuf>,
	/// Print partial updates.
	#[arg(long)]
	pub print_partials: bool,
	/// Truncate printed text for ticks/partials (does not affect segment/final output).
	#[arg(long, value_name = "N", default_value_t = 200)]
	pub max_text_len: usize,
	/// Sherpa streaming chunk size, in milliseconds.
	#[arg(long, value_name = "MS", default_value_t = 170)]
	pub sherpa_chunk_ms: u32,
	/// Sherpa model directory.
	#[arg(
		long,
		value_name = "PATH",
		default_value = "models/sherpa-onnx-streaming-zipformer-en-2023-06-21"
	)]
	pub sherpa_model_path: PathBuf,
	/// Sherpa provider.
	#[arg(long, value_enum, value_name = "PROVIDER", default_value_t = SherpaProvider::Cpu)]
	pub sherpa_provider: SherpaProvider,
	/// Sherpa thread count.
	#[arg(long, value_name = "N", default_value_t = default_sherpa_threads())]
	pub sherpa_threads: i32,
	/// Sherpa decoding method.
	#[arg(long, value_enum, value_name = "METHOD", default_value_t = SherpaDecodingMethod::GreedySearch)]
	pub sherpa_decoding_method: SherpaDecodingMethod,
	/// Sherpa max active paths.
	#[arg(long, value_name = "N", default_value_t = 4)]
	pub sherpa_max_active_paths: i32,
	/// Force fp32 encoder/joiner.
	#[arg(long)]
	pub sherpa_prefer_fp32: bool,
	/// Use int8 decoder if available.
	#[arg(long)]
	pub sherpa_int8_decoder: bool,
	/// Whisper model file.
	#[arg(long, value_name = "PATH", default_value = "models/whisper/ggml-large-v3-turbo-q8_0.bin")]
	pub whisper_model_path: PathBuf,
	/// Whisper thread count override.
	#[arg(long, value_name = "N")]
	pub whisper_threads: Option<i32>,
	/// Whisper language (e.g., 'en', 'zh', or 'auto').
	#[arg(long, value_name = "CODE|auto", default_value = "en")]
	pub whisper_language: String,
	/// Whisper GPU mode.
	#[arg(
		long,
		value_enum,
		value_name = "MODE",
		default_value_t = WhisperGpuMode::Auto
	)]
	pub whisper_gpu: WhisperGpuMode,
	/// Rolling window size for whisper baseline (milliseconds).
	#[arg(long, value_name = "MS", default_value_t = 8000)]
	pub whisper_window_ms: u32,
	/// Tick step for whisper baseline (milliseconds).
	#[arg(long, value_name = "MS", default_value_t = 500)]
	pub whisper_step_ms: u32,
	/// Whisper greedy sampling best-of (1..=8). Higher can improve accuracy at a CPU cost.
	#[arg(long, value_name = "N", default_value_t = 5)]
	pub whisper_best_of: i32,
	/// Whisper beam search size. When set, beam search is used instead of greedy sampling.
	#[arg(long, value_name = "N")]
	pub whisper_beam_size: Option<i32>,
	/// Whisper beam search patience. Only used when `--whisper-beam-size` is set.
	#[arg(long, value_name = "PATIENCE", default_value_t = -1.0)]
	pub whisper_beam_patience: f32,
	/// Print one tick every N steps for whisper-window when partials are disabled (0 disables).
	#[arg(long, value_name = "N", default_value_t = 5)]
	pub whisper_tick_every: u32,
}

pub enum InputPlan {
	Mic,
	Manifest { cases: Vec<ManifestCase> },
}

pub struct RunPlan {
	pub common: CommonConfig,
	pub input_plan: InputPlan,
}

pub fn parse_args() -> Result<RunPlan> {
	let Cli {
		mic,
		manifest,
		print_partials,
		max_text_len,
		sherpa_chunk_ms,
		sherpa_model_path,
		sherpa_provider,
		sherpa_threads,
		sherpa_decoding_method,
		sherpa_max_active_paths,
		sherpa_prefer_fp32,
		sherpa_int8_decoder,
		whisper_model_path,
		whisper_threads,
		whisper_language,
		whisper_gpu,
		whisper_window_ms,
		whisper_step_ms,
		whisper_best_of,
		whisper_beam_size,
		whisper_beam_patience,
		whisper_tick_every,
	} = Cli::parse();
	let sherpa = SherpaConfig {
		model_path: sherpa_model_path,
		provider: sherpa_provider.as_str().into(),
		num_threads: sherpa_threads,
		decoding_method: sherpa_decoding_method.as_str().into(),
		max_active_paths: sherpa_max_active_paths,
		prefer_int8: !sherpa_prefer_fp32,
		use_int8_decoder: sherpa_int8_decoder,
	};
	let whisper_language = whisper_language.trim().to_string();

	if whisper_language.is_empty() {
		return Err(eyre::eyre!("Whisper language must not be empty."));
	}
	if whisper_best_of < 1 {
		return Err(eyre::eyre!("--whisper-best-of must be at least 1."));
	}
	if whisper_best_of > 8 {
		return Err(eyre::eyre!("--whisper-best-of must be at most 8."));
	}
	if let Some(size) = whisper_beam_size
		&& size < 1
	{
		return Err(eyre::eyre!("--whisper-beam-size must be at least 1."));
	}

	let whisper_force_gpu = match whisper_gpu {
		WhisperGpuMode::Auto => None,
		WhisperGpuMode::On => Some(true),
		WhisperGpuMode::Off => Some(false),
	};
	let whisper = WhisperConfig {
		model_path: whisper_model_path,
		num_threads: whisper_threads,
		language: whisper_language,
		force_gpu: whisper_force_gpu,
		window_ms: whisper_window_ms,
		step_ms: whisper_step_ms,
		best_of: whisper_best_of,
		beam_size: whisper_beam_size,
		beam_patience: whisper_beam_patience,
	};
	let input_plan = if mic {
		InputPlan::Mic
	} else {
		let manifest_path = if let Some(path) = manifest {
			path
		} else {
			let default_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("manifest.toml");

			if !default_path.is_file() {
				return Err(eyre::eyre!("No input specified. Provide --mic or --manifest <PATH>."));
			}

			default_path
		};

		let cases = references::load_manifest_cases(&manifest_path)?;

		if cases.is_empty() {
			return Err(eyre::eyre!(
				"Manifest contains no runnable entries: {}.",
				manifest_path.display()
			));
		}

		InputPlan::Manifest { cases }
	};

	let common = CommonConfig {
		sherpa_chunk_ms,
		print_partials,
		whisper_tick_every,
		max_text_len,
		sherpa,
		whisper,
	};

	Ok(RunPlan { common, input_plan })
}

fn styles() -> Styles {
	Styles::styled()
		.header(AnsiColor::Red.on_default() | Effects::BOLD)
		.usage(AnsiColor::Red.on_default() | Effects::BOLD)
		.literal(AnsiColor::Blue.on_default() | Effects::BOLD)
		.placeholder(AnsiColor::Green.on_default())
}

fn default_sherpa_threads() -> i32 {
	let cpu = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
	let half = cpu / 2;

	i32::try_from(half.max(1)).unwrap_or(1)
}
