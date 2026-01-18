use inkflow_core::{InkFlowEngine, SttSettings};

#[test]
fn engine_initializes_with_local_assets() {
	if std::env::var("INKFLOW_STT_SMOKE_TEST").as_deref() != Ok("1") {
		eprintln!("Skipping STT smoke test. Set INKFLOW_STT_SMOKE_TEST=1 to run.");
		return;
	}

	let result = InkFlowEngine::start(SttSettings::default());
	match result {
		Ok(engine) =>
			if let Err(err) = engine.stop() {
				panic!("Engine shutdown failed: {}.", err.message);
			},
		Err(err) => {
			panic!("Engine initialization failed: {}.", err.message);
		},
	}
}
