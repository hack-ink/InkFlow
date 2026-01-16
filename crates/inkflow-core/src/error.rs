use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AppError {
	pub code: String,
	pub message: String,
}

impl AppError {
	pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
		Self { code: code.into(), message: message.into() }
	}
}
