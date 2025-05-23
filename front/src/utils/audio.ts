import timer from "../assets/timer.mp3?url";
import crash from "../assets/crash.mp3?url";
import cashout from "../assets/cashout.mp3?url";
import tick from "../assets/tick.mp3?url";

let audioContext: AudioContext | null = null;
const soundBuffers: Record<string, AudioBuffer | null> = {
    timer: null,
    crash: null,
    cashout: null,
    tick: null,
};

export async function initAudio() {
    if (!audioContext) {
        audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
    }
    await Promise.all([
        loadSound("timer", timer),
        loadSound("crash", crash),
        loadSound("cashout", cashout),
        loadSound("tick", tick),
    ]);
}

export async function loadSound(name: string, url: string) {
    if (!audioContext) {
        audioContext = new (window.AudioContext || (window as any).webkitAudioContext)();
    }
    const response = await fetch(url);
    const arrayBuffer = await response.arrayBuffer();
    soundBuffers[name] = await audioContext.decodeAudioData(arrayBuffer);
}

export function playSound(name: string, volume = 1.0) {
    if (!audioContext) return;
    const buffer = soundBuffers[name];
    if (!buffer) return;
    const source = audioContext.createBufferSource();
    source.buffer = buffer;
    const gain = audioContext.createGain();
    gain.gain.value = volume;
    source.connect(gain).connect(audioContext.destination);
    source.start(0);
}

export function playLoopingSound(name: string, volume = 1.0, rate = 1.0) {
    if (!audioContext) return null;
    const buffer = soundBuffers[name];
    if (!buffer) return null;
    const source = audioContext.createBufferSource();
    source.buffer = buffer;
    source.loop = true;
    source.playbackRate.value = rate;
    const gain = audioContext.createGain();
    gain.gain.value = volume;
    source.connect(gain).connect(audioContext.destination);
    source.start(0);
    return {
        stop: () => source.stop(),
        setVolume: (v: number) => {
            gain.gain.value = v;
        },
        setRate: (r: number) => {
            source.playbackRate.value = r;
        },
    };
}

export function closeAudio() {
    if (audioContext) {
        audioContext.close();
        audioContext = null;
    }
}

initAudio();
