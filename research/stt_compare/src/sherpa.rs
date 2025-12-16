// self
use crate::prelude::*;

pub struct SherpaModelFiles {
	pub tokens: PathBuf,
	pub encoder: PathBuf,
	pub joiner: PathBuf,
	pub decoder: PathBuf,
}

pub fn resolve_sherpa_model_files(
	model_dir: &Path,
	prefer_int8: bool,
	use_int8_decoder: bool,
) -> Result<SherpaModelFiles> {
	let tokens = model_dir.join("tokens.txt");

	if !tokens.is_file() {
		return Err(eyre::eyre!("Missing required sherpa model file: {}.", tokens.display()));
	}

	let encoder = if prefer_int8 {
		let candidate = model_dir.join("encoder-epoch-99-avg-1.int8.onnx");

		if candidate.is_file() { candidate } else { model_dir.join("encoder-epoch-99-avg-1.onnx") }
	} else {
		model_dir.join("encoder-epoch-99-avg-1.onnx")
	};
	let joiner = if prefer_int8 {
		let candidate = model_dir.join("joiner-epoch-99-avg-1.int8.onnx");

		if candidate.is_file() { candidate } else { model_dir.join("joiner-epoch-99-avg-1.onnx") }
	} else {
		model_dir.join("joiner-epoch-99-avg-1.onnx")
	};
	let decoder = if use_int8_decoder {
		let candidate = model_dir.join("decoder-epoch-99-avg-1.int8.onnx");

		if candidate.is_file() { candidate } else { model_dir.join("decoder-epoch-99-avg-1.onnx") }
	} else {
		model_dir.join("decoder-epoch-99-avg-1.onnx")
	};
	let required = [(&encoder, "encoder"), (&joiner, "joiner"), (&decoder, "decoder")];
	let missing = required
		.into_iter()
		.filter_map(
			|(p, label)| {
				if p.is_file() { None } else { Some(format!("{label}: {}", p.display())) }
			},
		)
		.collect::<Vec<_>>();

	if !missing.is_empty() {
		let joined = missing.join("\n- ");

		return Err(eyre::eyre!("Missing required sherpa model files:\n- {joined}"));
	}

	Ok(SherpaModelFiles { tokens, encoder, joiner, decoder })
}
