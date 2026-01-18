// std
use std::{env, path::PathBuf};
// crates.io
#[allow(deprecated)] use bindgen::CargoCallbacks;

fn main() {
	println!("cargo:rerun-if-changed=vendor/sherpa_onnx_c_api.h");

	let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
	let header = manifest_dir.join("vendor/sherpa_onnx_c_api.h");
	let bindings = bindgen::builder()
		.header(header.to_string_lossy())
		.allowlist_function("SherpaOnnx.*Online.*")
		.allowlist_type("SherpaOnnx.*Online.*")
		.allowlist_type("SherpaOnnxFeatureConfig")
		.allowlist_type("SherpaOnnxOnlineCtcFstDecoderConfig")
		.allowlist_type("SherpaOnnxHomophoneReplacerConfig")
		.allowlist_type("SherpaOnnxOnlineRecognizerConfig")
		.allowlist_type("SherpaOnnxOnlineModelConfig")
		.allowlist_type("SherpaOnnxOnlineTransducerModelConfig")
		.layout_tests(false)
		.generate_comments(false)
		.parse_callbacks(Box::new(CargoCallbacks::new()))
		.generate();
	let bindings = match bindings {
		Ok(bindings) => bindings,
		Err(e) => {
			panic!("Failed to generate sherpa-onnx bindings: {e}.");
		},
	};
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap_or_default());
	let out_file = out_dir.join("bindings.rs");

	if let Err(e) = bindings.write_to_file(&out_file) {
		panic!("Failed to write sherpa-onnx bindings to {out_file:?}: {e}.");
	}
}
