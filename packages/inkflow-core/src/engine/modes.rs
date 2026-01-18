use crate::settings::SttSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodeMode {
	StreamSecondPass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InferenceMode {
	LocalOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PipelinePlan {
	pub(crate) decode_mode: DecodeMode,
	pub(crate) inference: InferenceMode,
	pub(crate) window_enabled: bool,
}

pub(crate) struct ModeRouter;

impl ModeRouter {
	pub(crate) fn resolve(settings: &SttSettings) -> PipelinePlan {
		PipelinePlan {
			decode_mode: DecodeMode::StreamSecondPass,
			inference: InferenceMode::LocalOnly,
			window_enabled: settings.window.enabled,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mode_router_defaults_to_local_stream_second_pass() {
		let settings = SttSettings::default();
		let plan = ModeRouter::resolve(&settings);
		assert_eq!(plan.decode_mode, DecodeMode::StreamSecondPass);
		assert_eq!(plan.inference, InferenceMode::LocalOnly);
		assert_eq!(plan.window_enabled, settings.window.enabled);
	}
}
