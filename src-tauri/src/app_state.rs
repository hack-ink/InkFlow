use std::sync::Arc;

use crate::{engine::EngineManager, session::SessionManager, settings::SettingsStore};

pub struct AppState {
	session: SessionManager,
	settings: Arc<SettingsStore>,
	engine: Arc<EngineManager>,
}

impl AppState {
	pub fn new() -> Self {
		let settings = Arc::new(SettingsStore::new());
		let engine = Arc::new(EngineManager::new());
		let session = SessionManager::new(engine.clone(), settings.clone());
		Self { session, settings, engine }
	}

	pub fn session(&self) -> &SessionManager {
		&self.session
	}

	pub fn settings(&self) -> &SettingsStore {
		self.settings.as_ref()
	}

	pub fn engine(&self) -> &EngineManager {
		self.engine.as_ref()
	}
}
