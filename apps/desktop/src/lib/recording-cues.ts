type CueTone = {
  frequency: number;
  durationSeconds: number;
  gain: number;
};

type WindowWithWebkitAudioContext = Window &
  typeof globalThis & {
    webkitAudioContext?: typeof AudioContext;
  };

let sharedAudioContext: AudioContext | null = null;

function clampVolume(volume: number): number {
  if (!Number.isFinite(volume)) {
    return 0;
  }

  return Math.min(100, Math.max(0, volume));
}

function getAudioContext(): AudioContext | null {
  if (typeof window === "undefined") {
    return null;
  }

  const audioWindow = window as WindowWithWebkitAudioContext;
  const AudioContextConstructor =
    audioWindow.AudioContext ?? audioWindow.webkitAudioContext;

  if (!AudioContextConstructor) {
    return null;
  }

  if (!sharedAudioContext) {
    sharedAudioContext = new AudioContextConstructor();
  }

  return sharedAudioContext;
}

async function playCue(tones: CueTone[], volume: number): Promise<void> {
  const normalizedVolume = (clampVolume(volume) / 100) * 2;
  if (normalizedVolume <= 0) {
    return;
  }

  const context = getAudioContext();
  if (!context) {
    return;
  }

  if (context.state === "suspended") {
    try {
      await context.resume();
    } catch {
      return;
    }
  }

  const startAt = context.currentTime + 0.01;
  let offset = 0;

  for (const tone of tones) {
    const oscillator = context.createOscillator();
    const gainNode = context.createGain();
    const toneStart = startAt + offset;
    const toneEnd = toneStart + tone.durationSeconds;

    oscillator.type = "sine";
    oscillator.frequency.setValueAtTime(tone.frequency, toneStart);

    gainNode.gain.setValueAtTime(0.0001, toneStart);
    gainNode.gain.exponentialRampToValueAtTime(
      tone.gain * normalizedVolume,
      toneStart + 0.01,
    );
    gainNode.gain.exponentialRampToValueAtTime(0.0001, toneEnd);

    oscillator.connect(gainNode);
    gainNode.connect(context.destination);
    oscillator.start(toneStart);
    oscillator.stop(toneEnd);

    offset += tone.durationSeconds + 0.02;
  }
}

export async function playRecordingStartCue(volume: number): Promise<void> {
  await playCue([
    { frequency: 880, durationSeconds: 0.06, gain: 0.09 },
    { frequency: 1174, durationSeconds: 0.08, gain: 0.075 },
  ], volume);
}

export async function playRecordingStopCue(volume: number): Promise<void> {
  await playCue([
    { frequency: 1046, durationSeconds: 0.05, gain: 0.085 },
    { frequency: 784, durationSeconds: 0.08, gain: 0.07 },
  ], volume);
}
