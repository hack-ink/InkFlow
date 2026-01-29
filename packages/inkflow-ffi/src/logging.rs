use std::{
	fs,
	path::{Path, PathBuf},
	sync::OnceLock,
	time::{Duration, SystemTime},
};

use directories::ProjectDirs;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, registry::Registry};

#[derive(Debug, Clone)]
pub(crate) struct LoggingConfig {
	pub(crate) default_level: String,
	pub(crate) outputs: Vec<LogOutput>,
}

impl Default for LoggingConfig {
	fn default() -> Self {
		Self {
			default_level: "info".to_string(),
			outputs: vec![LogOutput::File(FileOutputConfig::default())],
		}
	}
}

#[derive(Debug, Clone)]
pub(crate) enum LogOutput {
	File(FileOutputConfig),
}

#[derive(Debug, Clone)]
pub(crate) struct FileOutputConfig {
	pub(crate) dir: Option<PathBuf>,
	pub(crate) filename: String,
	pub(crate) rotation: Rotation,
	pub(crate) level_override: Option<String>,
}

impl Default for FileOutputConfig {
	fn default() -> Self {
		Self {
			dir: None,
			filename: "inkflow.log".to_string(),
			rotation: Rotation::Daily,
			level_override: None,
		}
	}
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rotation {
	Daily,
	Hourly,
	Never,
}

struct LoggingHandle {
	_guards: Vec<WorkerGuard>,
}

static LOG_HANDLE: OnceLock<LoggingHandle> = OnceLock::new();

pub(crate) fn init() {
	init_with(LoggingConfig::default());
}

pub(crate) fn init_with(config: LoggingConfig) {
	if LOG_HANDLE.get().is_some() {
		return;
	}

	let base_filter = EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| EnvFilter::new(config.default_level.clone()));
	let (layers, guards) = build_layers(&config, &base_filter);
	if layers.is_empty() {
		return;
	}

	let subscriber = tracing_subscriber::registry().with(layers);
	if tracing::subscriber::set_global_default(subscriber).is_ok() {
		let _ = LOG_HANDLE.set(LoggingHandle { _guards: guards });
	}
}

fn build_layers(
	config: &LoggingConfig,
	base_filter: &EnvFilter,
) -> (Vec<Box<dyn Layer<Registry> + Send + Sync>>, Vec<WorkerGuard>) {
	let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();
	let mut guards = Vec::new();

	for output in &config.outputs {
		match output {
			LogOutput::File(file_config) => {
				let filter = output_filter(base_filter, file_config.level_override.as_deref());
				let (layer, guard) = build_file_layer(file_config, filter);
				layers.push(layer);
				guards.push(guard);
			},
		}
	}

	(layers, guards)
}

fn output_filter(base: &EnvFilter, override_level: Option<&str>) -> EnvFilter {
	match override_level {
		Some(level) => EnvFilter::new(level),
		None => base.clone(),
	}
}

fn build_file_layer(
	config: &FileOutputConfig,
	filter: EnvFilter,
) -> (Box<dyn Layer<Registry> + Send + Sync>, WorkerGuard) {
	let log_dir = resolve_log_dir(config);
	prune_old_logs(&log_dir, &config.filename, Duration::from_secs(7 * 24 * 60 * 60));
	let appender = match config.rotation {
		Rotation::Daily => tracing_appender::rolling::daily(&log_dir, &config.filename),
		Rotation::Hourly => tracing_appender::rolling::hourly(&log_dir, &config.filename),
		Rotation::Never => tracing_appender::rolling::never(&log_dir, &config.filename),
	};
	let (non_blocking, guard) = tracing_appender::non_blocking(appender);

	let layer = fmt::layer()
		.with_ansi(false)
		.with_level(true)
		.with_target(true)
		.with_thread_names(true)
		.with_thread_ids(true)
		.with_writer(non_blocking)
		.with_filter(filter);

	(Box::new(layer), guard)
}

fn resolve_log_dir(config: &FileOutputConfig) -> PathBuf {
	let fallback_root = std::env::temp_dir().join("InkFlow");

	if let Some(dir) = config.dir.as_ref() {
		return ensure_log_dir(dir, &fallback_root);
	}

	let project = ProjectDirs::from("ink", "hack", "InkFlow");
	let dir = build_log_dir(project.as_ref(), &fallback_root);
	ensure_log_dir(&dir, &fallback_root)
}

fn ensure_log_dir(dir: &Path, fallback_root: &Path) -> PathBuf {
	if fs::create_dir_all(dir).is_ok() {
		return dir.to_path_buf();
	}

	let fallback = fallback_root.join("logs");
	let _ = fs::create_dir_all(&fallback);
	fallback
}

fn build_log_dir(project: Option<&ProjectDirs>, fallback_root: &Path) -> PathBuf {
	match project {
		Some(project) => project.data_local_dir().join("logs"),
		None => fallback_root.join("logs"),
	}
}

fn prune_old_logs(log_dir: &Path, filename: &str, max_age: Duration) {
	prune_old_logs_at(log_dir, filename, max_age, SystemTime::now());
}

fn prune_old_logs_at(log_dir: &Path, filename: &str, max_age: Duration, now: SystemTime) {
	let cutoff = match now.checked_sub(max_age) {
		Some(cutoff) => cutoff,
		None => return,
	};
	let Ok(entries) = fs::read_dir(log_dir) else {
		return;
	};

	for entry in entries.flatten() {
		let path = entry.path();
		let Ok(meta) = entry.metadata() else {
			continue;
		};
		if !meta.is_file() {
			continue;
		}
		let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
			continue;
		};
		if !name.starts_with(filename) {
			continue;
		}
		let Ok(modified) = meta.modified() else {
			continue;
		};
		if modified < cutoff {
			let _ = fs::remove_file(&path);
		}
	}
}

#[cfg(test)]
mod tests {
	use std::{
		path::PathBuf,
		time::{Duration, SystemTime, UNIX_EPOCH},
	};

	use directories::ProjectDirs;

	use super::{
		FileOutputConfig, LogOutput, LoggingConfig, Rotation, build_log_dir, prune_old_logs_at,
	};

	#[test]
	fn default_config_includes_file_output() {
		let config = LoggingConfig::default();
		assert_eq!(config.outputs.len(), 1);
		match &config.outputs[0] {
			LogOutput::File(file) => {
				assert_eq!(file.rotation, Rotation::Daily);
				assert!(file.level_override.is_none());
			},
		}
	}

	#[test]
	fn file_output_default_filename() {
		let file = FileOutputConfig::default();
		assert_eq!(file.filename, "inkflow.log");
	}

	#[test]
	fn build_log_dir_uses_fallback_when_project_missing() {
		let fallback = PathBuf::from("/tmp/inkflow-logging");
		let dir = build_log_dir(None, &fallback);
		assert_eq!(dir, fallback.join("logs"));
	}

	#[test]
	fn build_log_dir_uses_project_data_dir_when_available() {
		let Some(project) = ProjectDirs::from("ink", "hack", "InkFlow") else {
			return;
		};
		let fallback = PathBuf::from("/tmp/inkflow-logging");
		let dir = build_log_dir(Some(&project), &fallback);
		assert_eq!(dir, project.data_local_dir().join("logs"));
	}

	#[test]
	fn prune_old_logs_removes_files_older_than_cutoff() {
		let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
		let root = std::env::temp_dir().join(format!("inkflow-logs-{stamp}"));
		std::fs::create_dir_all(&root).unwrap();

		let old_path = root.join("inkflow.log.2020-01-01");
		let new_path = root.join("inkflow.log.2020-01-02");
		std::fs::write(&old_path, "old").unwrap();
		std::thread::sleep(Duration::from_secs(2));
		std::fs::write(&new_path, "new").unwrap();

		let old_time = std::fs::metadata(&old_path).unwrap().modified().unwrap();
		let now = old_time + Duration::from_secs(7 * 24 * 60 * 60 + 1);

		prune_old_logs_at(&root, "inkflow.log", Duration::from_secs(7 * 24 * 60 * 60), now);

		assert!(!old_path.exists());
		assert!(new_path.exists());

		let _ = std::fs::remove_dir_all(&root);
	}
}
