mod accuracy;
mod audio;
mod cli;
mod config;
mod mic;
mod mic_capture;
mod references;
mod sherpa;
mod twopass;
mod util;
mod whisper;
mod whisper_window;
mod prelude {
	pub use std::path::{Path, PathBuf};

	pub use color_eyre::eyre::{self, Result, WrapErr};
}

// std
use std::fs;
// crates.io
use tracing_subscriber::EnvFilter;
// self
use cli::{InputPlan, RunPlan};
use config::RunConfig;
use prelude::*;

fn main() -> Result<()> {
	color_eyre::install()?;
	tracing_subscriber::fmt()
		.with_env_filter(
			EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
		)
		.init();
	whisper_rs::install_logging_hooks();

	let RunPlan { common, input_plan } = cli::parse_args()?;

	match input_plan {
		InputPlan::Mic => {
			let captured = mic_capture::capture_to_temp_wav()?;
			let captured_wav_path = captured.wav_path.clone();
			let run = RunConfig { wav_path: captured.wav_path, reference_text: None };
			let result = (|| {
				twopass::run(&common, &run)?;

				println!();

				whisper_window::run(&common, &run)?;

				Result::Ok(())
			})();

			if let Err(e) = fs::remove_file(&captured_wav_path) {
				eprintln!("Failed to remove temporary WAV file: {e}.");
			}

			return result;
		},
		InputPlan::Manifest { cases } =>
			for (idx, case) in cases.into_iter().enumerate() {
				println!("[case] idx={} wav={}", idx + 1, case.wav_path.display());

				let run =
					RunConfig { wav_path: case.wav_path, reference_text: case.reference_text };

				twopass::run(&common, &run)?;

				println!();

				whisper_window::run(&common, &run)?;

				println!();
			},
	}

	Ok(())
}
