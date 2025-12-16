# sherpa-onnx demo (Poetry + .venv)

This project is a minimal demo for the `k2-fsa/sherpa-onnx` Python API:

- Uses `poetry` and an in-project virtualenv (`.venv/`).
- Automatically downloads a streaming English Zipformer-Transducer model into `<repo>/model/` by default.
- Does not use the system `pip3`.

## Requirements

- Python 3.10+
- Poetry 2+

## Run

```bash
cargo make sherpa-demo
```

The first run will:

1. Create `.venv/` and install dependencies via Poetry.
2. Download and extract the model to `<repo>/model/`.
3. Run streaming ASR on the model's included sample WAV.

## Microphone mode (optional)

Microphone capture depends on `sounddevice` (and PortAudio).

```bash
poetry add sounddevice
cargo make sherpa-demo-mic
```

## Notes

- Default model: `sherpa-onnx-streaming-zipformer-en-2023-06-21` (downloaded from GitHub releases).
- To force re-download, delete `<repo>/model/` and run again.
