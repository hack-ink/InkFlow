use std::sync::{
	Arc,
	atomic::{AtomicBool, Ordering},
};

use tokio::sync::oneshot;

use crate::{
	adapters::{PlatformAdapter, UiAdapter},
	engine::EngineManager,
	error::AppError,
	ports::{PlatformPort, UiPort},
	settings::SettingsStore,
};

use super::session_actor::{
	SessionAction, SessionActor, SessionCommand, SessionContext, SessionSnapshot, SessionState,
};

pub struct SessionService {
	command_tx: tokio::sync::mpsc::Sender<SessionCommand>,
	state_rx: tokio::sync::watch::Receiver<SessionState>,
	engine: Arc<EngineManager>,
	settings: Arc<SettingsStore>,
	platform: Arc<PlatformAdapter>,
	attached: AtomicBool,
}
impl SessionService {
	pub fn new(engine: Arc<EngineManager>, settings: Arc<SettingsStore>) -> Self {
		let (command_tx, state_rx) = SessionActor::spawn();
		Self {
			command_tx,
			state_rx,
			engine,
			settings,
			platform: Arc::new(PlatformAdapter::new()),
			attached: AtomicBool::new(false),
		}
	}

	pub async fn dispatch(
		&self,
		app: &tauri::AppHandle,
		action: SessionAction,
	) -> Result<SessionSnapshot, AppError> {
		self.ensure_context(app).await?;

		let (reply_tx, reply_rx) = oneshot::channel::<Result<SessionSnapshot, AppError>>();
		self.command_tx
			.send(SessionCommand::UserAction { action, reply: reply_tx })
			.await
			.map_err(|_| AppError::new("session_unavailable", "Session service is unavailable."))?;

		reply_rx
			.await
			.map_err(|_| AppError::new("session_unavailable", "Session service is unavailable."))?
	}

	pub async fn is_listening_or_finalizing(&self) -> bool {
		matches!(*self.state_rx.borrow(), SessionState::Listening | SessionState::Finalizing)
	}

	async fn ensure_context(&self, app: &tauri::AppHandle) -> Result<(), AppError> {
		if self.attached.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err()
		{
			return Ok(());
		}

		let ui: Arc<dyn UiPort> = Arc::new(UiAdapter::new(app.clone()));
		let platform: Arc<dyn PlatformPort> = self.platform.clone();
		let context = SessionContext::new(
			app.clone(),
			self.engine.clone(),
			self.settings.clone(),
			ui,
			platform,
		);

		if self.command_tx.send(SessionCommand::AttachContext(Arc::new(context))).await.is_err() {
			self.attached.store(false, Ordering::SeqCst);
			return Err(AppError::new("session_unavailable", "Session service is unavailable."));
		}

		Ok(())
	}
}
