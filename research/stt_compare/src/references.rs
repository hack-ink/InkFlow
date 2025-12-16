// crates.io
use serde::Deserialize;
// self
use crate::prelude::*;

#[derive(Debug, Deserialize)]
pub struct ReferenceManifest {
	#[serde(default)]
	pub references: Vec<ReferenceEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ReferenceEntry {
	pub wav: PathBuf,
	pub text: String,
}

#[derive(Clone, Debug)]
pub struct ManifestCase {
	pub wav_path: PathBuf,
	pub reference_text: Option<String>,
}

pub fn load_manifest_cases(manifest_path: &Path) -> Result<Vec<ManifestCase>> {
	let content = std::fs::read_to_string(manifest_path)
		.wrap_err_with(|| format!("Failed to read manifest file: {}.", manifest_path.display()))?;
	let manifest = toml::from_str::<ReferenceManifest>(&content)
		.wrap_err_with(|| format!("Failed to parse manifest TOML: {}.", manifest_path.display()))?;
	let mut cases = Vec::new();

	for entry in manifest.references {
		let wav_path = resolve_wav_path(manifest_path, &entry.wav)?;

		if !wav_path.is_file() {
			return Err(eyre::eyre!("WAV path is not a file: {}.", wav_path.display()));
		}

		let trimmed = entry.text.trim();
		let reference_text = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };

		cases.push(ManifestCase { wav_path, reference_text });
	}

	Ok(cases)
}

fn resolve_wav_path(manifest_path: &Path, wav: &Path) -> Result<PathBuf> {
	if wav.is_absolute() {
		return Ok(wav.to_path_buf());
	}

	let mut candidates: Vec<PathBuf> = Vec::new();

	if let Some(dir) = manifest_path.parent() {
		candidates.push(dir.join(wav));
	}
	if let Ok(cwd) = std::env::current_dir() {
		candidates.push(cwd.join(wav));
	}

	for candidate in &candidates {
		if candidate.is_file() {
			return Ok(candidate.to_path_buf());
		}
	}

	let mut message = String::new();

	message.push_str("WAV path not found.\n");
	message.push_str(&format!("manifest={}\n", manifest_path.display()));
	message.push_str(&format!("wav={}\n", wav.display()));

	if !candidates.is_empty() {
		message.push_str("Tried:\n");

		for candidate in candidates {
			message.push_str("- ");
			message.push_str(&candidate.display().to_string());
			message.push('\n');
		}
	}

	Err(eyre::eyre!(message))
}
