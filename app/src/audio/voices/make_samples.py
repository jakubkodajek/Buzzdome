"""Renders the voice samples the settings play before the voice model is
downloaded: one Czech and one English sentence in each of the model's
voices, as `<language>-<voice>.mp3`.

Usage: python3 make_samples.py <unpacked model folder>
Needs `pip install sherpa-onnx numpy` and an ffmpeg with libmp3lame.
"""

import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np
import sherpa_onnx

TEXTS: dict[str, str] = {
    "cs": "Vítejte u kvízu! Která řeka protéká Prahou? A: Vltava. B: Labe.",
    "en": "Welcome to the quiz! Which river flows through London? A: The Thames. B: The Severn.",
}
VOICES = 10  # quiz_core::MAX_VOICE + 1 (crates/quiz-core/src/settings.rs)
# As the app renders (crates/voice/src/engine.rs).
STEPS = 8


def engine(model: Path) -> sherpa_onnx.OfflineTts:
    files = [
        "duration_predictor.int8.onnx",
        "text_encoder.int8.onnx",
        "vector_estimator.int8.onnx",
        "vocoder.int8.onnx",
        "tts.json",
        "unicode_indexer.bin",
        "voice.bin",
    ]
    config = sherpa_onnx.OfflineTtsModelConfig()
    config.supertonic = sherpa_onnx.OfflineTtsSupertonicModelConfig(*(str(model / f) for f in files))
    config.num_threads = 2
    config.provider = "cpu"
    return sherpa_onnx.OfflineTts(sherpa_onnx.OfflineTtsConfig(model=config))


def main() -> None:
    tts = engine(Path(sys.argv[1]))
    here = Path(__file__).parent
    with tempfile.TemporaryDirectory() as tmp:
        for language, text in TEXTS.items():
            for voice in range(VOICES):
                generation = sherpa_onnx.GenerationConfig()
                generation.sid = voice
                generation.num_steps = STEPS
                generation.extra = {"lang": language}
                audio = tts.generate(text, generation)
                pcm = (np.clip(np.array(audio.samples), -1, 1) * 32767).astype("<i2")
                wav = Path(tmp) / "sample.wav"
                with wave.open(str(wav), "wb") as out:
                    out.setnchannels(1)
                    out.setsampwidth(2)
                    out.setframerate(audio.sample_rate)
                    out.writeframes(pcm.tobytes())
                mp3 = here / f"{language}-{voice}.mp3"
                subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(wav), "-ac", "1", "-ar", "24000", "-b:a", "48k", str(mp3)], check=True)


if __name__ == "__main__":
    main()
