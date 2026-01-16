# STT Dictation Pipeline – Implementation Notes

Date: 2026-01-16

This document tracks the current SwiftUI + Rust FFI implementation of the dictation pipeline.
The canonical behavior specification lives in `docs/spec/stt_dictation_pipeline.md`.

## Current Implementation Locations

- Engine entry point: `crates/inkflow-core/src/engine.rs`.
- STT adapters: `crates/inkflow-core/src/stt/*`.
- Transcript domain model: `crates/inkflow-core/src/domain/*`.
- FFI boundary: `crates/inkflow-ffi/src/lib.rs` and `crates/inkflow-ffi/include/inkflow.h`.

## Status

- The SwiftUI client sends audio frames through the FFI callback API.
- The Rust engine owns transcription state and emits JSON updates.
- Legacy Tauri implementation notes have been removed after the SwiftUI migration.

## Verification

- Build the Rust engine with `cargo build -p inkflow-core`.
- Build the macOS app with `xcodebuild -project apps/macos/InkFlow/InkFlow.xcodeproj -scheme InkFlow -configuration Debug build`.
