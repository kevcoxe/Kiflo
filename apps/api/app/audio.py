from __future__ import annotations

import io
import wave


def estimate_audio_duration_ms(audio_bytes: bytes) -> int:
    if not audio_bytes:
        return 0

    try:
        with wave.open(io.BytesIO(audio_bytes), "rb") as wav_file:
            frame_count = wav_file.getnframes()
            frame_rate = wav_file.getframerate()
            if frame_rate <= 0:
                return 0
            return int((frame_count / frame_rate) * 1000)
    except wave.Error:
        # The first slice only supports WAV end-to-end. If a different payload
        # shows up, fall back to a rough estimate so the response shape stays stable.
        return min(len(audio_bytes) // 32, 30_000)
