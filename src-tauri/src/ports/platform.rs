use crate::error::AppError;

pub(crate) trait PlatformPort
where
	Self: Send + Sync,
{
	fn inject_text_via_paste(&self, text: &str) -> Result<(), AppError>;
}
