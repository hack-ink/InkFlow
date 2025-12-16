from __future__ import annotations

import argparse
import os
import sys
import tarfile
import urllib.request
import wave
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import numpy as np
import sherpa_onnx


MODEL_ID_DEFAULT = "sherpa-onnx-streaming-zipformer-en-2023-06-21"
MODEL_URL_DEFAULT = (
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/"
    "sherpa-onnx-streaming-zipformer-en-2023-06-21.tar.bz2"
)


@dataclass(frozen=True)
class ModelFiles:
    tokens: Path
    encoder: Path
    decoder: Path
    joiner: Path
    sample_wav: Path | None


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
        description="Streaming ASR demo using sherpa-onnx (model auto-download).",
    )

    parser.add_argument(
        "--models-dir",
        type=Path,
        default=Path(__file__).resolve().parents[3] / "model",
        help="Directory to store downloaded and extracted models.",
    )

    parser.add_argument(
        "--model-id",
        type=str,
        default=MODEL_ID_DEFAULT,
        help="Model directory name (after extraction).",
    )

    parser.add_argument(
        "--model-url",
        type=str,
        default=MODEL_URL_DEFAULT,
        help="URL to download the model tarball.",
    )

    parser.add_argument(
        "--mic",
        action="store_true",
        help="Use the microphone as input (requires sounddevice).",
    )

    parser.add_argument(
        "--wav",
        type=Path,
        default=None,
        help="Optional WAV file to transcribe. If not set, uses the model's sample WAV if present.",
    )

    parser.add_argument(
        "--provider",
        type=str,
        default="cpu",
        help="ONNX Runtime provider: cpu, cuda, or coreml.",
    )

    parser.add_argument(
        "--decoding-method",
        type=str,
        default="greedy_search",
        help="Decoding method: greedy_search or modified_beam_search.",
    )

    parser.add_argument(
        "--num-threads",
        type=int,
        default=max(1, (os.cpu_count() or 1) // 2),
        help="Number of CPU threads for inference.",
    )

    parser.add_argument(
        "--sample-rate",
        type=int,
        default=48000,
        help="Audio sample rate for microphone capture (sherpa-onnx will resample internally).",
    )

    parser.add_argument(
        "--chunk-seconds",
        type=float,
        default=0.1,
        help="Chunk size in seconds for WAV streaming.",
    )

    parser.add_argument(
        "--use-fp32",
        action="store_true",
        help="Use fp32 encoder/joiner if available instead of int8.",
    )

    return parser.parse_args(argv)


def _download_with_progress(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".partial")
    last_percent = -1

    def reporthook(blocks: int, block_size: int, total_size: int) -> None:
        nonlocal last_percent
        if total_size <= 0:
            return
        downloaded = min(blocks * block_size, total_size)
        percent = int(downloaded * 100 / total_size)
        if percent == last_percent:
            return
        last_percent = percent
        print(f"\rDownloading {dest.name}: {percent:3d}% ({downloaded}/{total_size} bytes)", end="")

    print(f"Downloading model from: {url}")
    try:
        urllib.request.urlretrieve(url, tmp, reporthook=reporthook)
    except Exception:
        print("\nDownload failed.")
        if tmp.exists():
            tmp.unlink(missing_ok=True)
        raise
    print()
    tmp.replace(dest)


def _extract_tar_bz2(tar_path: Path, dest_dir: Path) -> None:
    dest_dir.mkdir(parents=True, exist_ok=True)
    print(f"Extracting {tar_path.name} into {dest_dir}")
    with tarfile.open(tar_path, mode="r:bz2") as tar:
        dest_root = dest_dir.resolve()
        for member in tar.getmembers():
            if member.islnk() or member.issym():
                raise RuntimeError(f"Refusing to extract symlink from tar archive: {member.name}")

            member_path = (dest_root / member.name).resolve()
            if dest_root == member_path:
                continue
            if dest_root not in member_path.parents:
                raise RuntimeError(f"Refusing to extract outside destination directory: {member.name}")

        tar.extractall(dest_dir)


def _expected_model_files(model_dir: Path, use_fp32: bool) -> ModelFiles:
    tokens = model_dir / "tokens.txt"

    if use_fp32:
        encoder = model_dir / "encoder-epoch-99-avg-1.onnx"
        joiner = model_dir / "joiner-epoch-99-avg-1.onnx"
    else:
        encoder = model_dir / "encoder-epoch-99-avg-1.int8.onnx"
        joiner = model_dir / "joiner-epoch-99-avg-1.int8.onnx"

    decoder = model_dir / "decoder-epoch-99-avg-1.onnx"

    sample_candidates = [
        model_dir / "test_wavs" / "0.wav",
        model_dir / "test_wavs" / "1.wav",
        model_dir / "test_wavs" / "test.wav",
    ]
    sample_wav = next((p for p in sample_candidates if p.is_file()), None)

    return ModelFiles(tokens=tokens, encoder=encoder, decoder=decoder, joiner=joiner, sample_wav=sample_wav)


def _ensure_model(model_id: str, model_url: str, models_dir: Path, use_fp32: bool) -> ModelFiles:
    model_dir = models_dir / model_id
    files = _expected_model_files(model_dir=model_dir, use_fp32=use_fp32)

    if files.tokens.is_file() and files.encoder.is_file() and files.decoder.is_file() and files.joiner.is_file():
        return files

    tar_name = f"{model_id}.tar.bz2"
    tar_path = models_dir / tar_name

    _download_with_progress(model_url, tar_path)
    _extract_tar_bz2(tar_path, models_dir)

    files = _expected_model_files(model_dir=model_dir, use_fp32=use_fp32)
    missing = [p for p in [files.tokens, files.encoder, files.decoder, files.joiner] if not p.is_file()]
    if missing:
        joined = "\n".join(f"- {p}" for p in missing)
        raise RuntimeError(f"Model extraction completed, but required files are missing:\n{joined}")

    tar_path.unlink(missing_ok=True)
    return files


def _create_recognizer(
    files: ModelFiles,
    provider: str,
    decoding_method: str,
    num_threads: int,
) -> sherpa_onnx.OnlineRecognizer:
    return sherpa_onnx.OnlineRecognizer.from_transducer(
        tokens=str(files.tokens),
        encoder=str(files.encoder),
        decoder=str(files.decoder),
        joiner=str(files.joiner),
        num_threads=num_threads,
        sample_rate=16000,
        feature_dim=80,
        enable_endpoint_detection=True,
        rule1_min_trailing_silence=2.4,
        rule2_min_trailing_silence=1.2,
        rule3_min_utterance_length=300,
        decoding_method=decoding_method,
        provider=provider,
    )


def _read_wav_mono_float32(path: Path) -> tuple[int, np.ndarray]:
    with wave.open(str(path), "rb") as f:
        if f.getnchannels() != 1:
            raise RuntimeError(f"WAV must be mono. Got {f.getnchannels()} channels: {path}")
        if f.getsampwidth() != 2:
            raise RuntimeError(f"WAV must be 16-bit PCM. Got {8 * f.getsampwidth()}-bit: {path}")
        sample_rate = f.getframerate()
        pcm = f.readframes(f.getnframes())

    data_i16 = np.frombuffer(pcm, dtype=np.int16)
    data_f32 = data_i16.astype(np.float32) / 32768.0
    return sample_rate, data_f32


def _iter_chunks(data: np.ndarray, chunk_size: int) -> Iterable[np.ndarray]:
    for i in range(0, data.shape[0], chunk_size):
        yield data[i : i + chunk_size]


def _run_wav_streaming(recognizer: sherpa_onnx.OnlineRecognizer, wav_path: Path, chunk_seconds: float) -> None:
    sample_rate, samples = _read_wav_mono_float32(wav_path)
    if sample_rate <= 0:
        raise RuntimeError(f"Invalid WAV sample rate: {sample_rate}")

    chunk_size = max(1, int(chunk_seconds * sample_rate))
    stream = recognizer.create_stream()
    display = sherpa_onnx.Display()

    print(f"Transcribing WAV (streaming): {wav_path}")
    print("Press Ctrl+C to stop.")

    for chunk in _iter_chunks(samples, chunk_size):
        stream.accept_waveform(sample_rate, chunk)
        while recognizer.is_ready(stream):
            recognizer.decode_stream(stream)

        result = recognizer.get_result(stream)
        display.update_text(result)
        display.display()

        if recognizer.is_endpoint(stream):
            if result:
                display.finalize_current_sentence()
                display.display()
            recognizer.reset(stream)

    result = recognizer.get_result(stream)
    if result:
        display.update_text(result)
        display.finalize_current_sentence()
        display.display()


def _run_mic_streaming(recognizer: sherpa_onnx.OnlineRecognizer, sample_rate: int) -> None:
    try:
        import sounddevice as sd
    except Exception as e:
        raise RuntimeError(
            "Microphone mode requires the 'sounddevice' package. Install it with: poetry add sounddevice"
        ) from e

    devices = sd.query_devices()
    if not devices:
        print("No microphone devices found.")
        return

    default_input_device_idx = sd.default.device[0]
    print(f'Using default microphone: {devices[default_input_device_idx]["name"]}')
    print("Started! Please speak. Press Ctrl+C to stop.")

    samples_per_read = int(0.1 * sample_rate)
    stream = recognizer.create_stream()
    display = sherpa_onnx.Display()

    with sd.InputStream(channels=1, dtype="float32", samplerate=sample_rate) as s:
        while True:
            samples, _ = s.read(samples_per_read)
            samples = samples.reshape(-1)
            stream.accept_waveform(sample_rate, samples)
            while recognizer.is_ready(stream):
                recognizer.decode_stream(stream)

            result = recognizer.get_result(stream)
            display.update_text(result)
            display.display()

            if recognizer.is_endpoint(stream):
                if result:
                    display.finalize_current_sentence()
                    display.display()
                recognizer.reset(stream)


def main(argv: list[str] | None = None) -> None:
    args = _parse_args(sys.argv[1:] if argv is None else argv)

    files = _ensure_model(
        model_id=args.model_id,
        model_url=args.model_url,
        models_dir=args.models_dir,
        use_fp32=args.use_fp32,
    )

    recognizer = _create_recognizer(
        files=files,
        provider=args.provider,
        decoding_method=args.decoding_method,
        num_threads=args.num_threads,
    )

    if args.mic:
        _run_mic_streaming(recognizer=recognizer, sample_rate=args.sample_rate)
        return

    wav_path = args.wav or files.sample_wav
    if wav_path is None:
        raise RuntimeError(
            "No WAV input was provided and no sample WAV was found in the model directory. "
            "Use --wav to specify a WAV file or use --mic for microphone mode."
        )

    _run_wav_streaming(recognizer=recognizer, wav_path=wav_path, chunk_seconds=args.chunk_seconds)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nCaught Ctrl+C. Exiting.")
