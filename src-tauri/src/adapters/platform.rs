use crate::{error::AppError, ports::PlatformPort};

pub(crate) struct PlatformAdapter;
impl PlatformAdapter {
	pub(crate) fn new() -> Self {
		Self
	}
}
impl PlatformPort for PlatformAdapter {
	fn inject_text_via_paste(&self, text: &str) -> Result<(), AppError> {
		crate::platform::current().inject_text_via_paste(text)
	}
}
