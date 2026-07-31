import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getBootstrapConfig, type UiMode } from "@/lib/bootstrap";
import type {
  WasmWorkerEvent,
  WasmWorkerRequest,
  WasmWorkerStatus,
} from "@/workers/wasm-tts-protocol";

const SAMPLE_RATE = 24000;
const DEFAULT_WASM_START_THRESHOLD_SEC = 0.22;
const DEFAULT_WASM_RESUME_THRESHOLD_SEC = 0.34;

const PRESET_VOICES = [
  "alba",
  "marius",
  "javert",
  "jean",
  "fantine",
  "cosette",
  "eponine",
  "azelma",
] as const;

const DEFAULT_CONFIG_YAML = `
flow_lm:
  dtype: float32
  flow:
    depth: 6
    dim: 512
  transformer:
    d_model: 1024
    hidden_scale: 4
    max_period: 10000
    num_heads: 16
    num_layers: 6
  lookup_table:
    dim: 1024
    n_bins: 4000
    tokenizer: sentencepiece
    tokenizer_path: hf://kyutai/pocket-tts-without-voice-cloning/tokenizer.model@d4fdd22ae8c8e1cb3634e150ebeff1dab2d16df3

mimi:
  dtype: float32
  sample_rate: 24000
  channels: 1
  frame_rate: 12.5
  seanet:
    dimension: 512
    channels: 1
    n_filters: 64
    n_residual_layers: 1
    ratios: [6, 5, 4]
    kernel_size: 7
    residual_kernel_size: 3
    last_kernel_size: 3
    dilation_base: 2
    pad_mode: constant
    compress: 2
  transformer:
    d_model: 512
    num_heads: 8
    num_layers: 2
    layer_scale: 0.01
    context: 250
    dim_feedforward: 2048
    input_dimension: 512
    output_dimensions: [512]
  quantizer:
    dimension: 32
    output_dimension: 512
`;

const WORKLET_CODE = `
class PCMProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.queue = [];
    this.queueOffset = 0;
    this.hasStarted = false;
    this.isBuffering = true;
    this.ended = false;
    this.firstAudioSent = false;
    this.tick = 0;
    this.underruns = 0;

    const startThreshold = options && options.processorOptions ? options.processorOptions.startThreshold : undefined;
    const resumeThreshold = options && options.processorOptions ? options.processorOptions.resumeThreshold : undefined;
    this.startThreshold = Number.isFinite(startThreshold) ? startThreshold : (24000 * 2.5);
    this.resumeThreshold = Number.isFinite(resumeThreshold) ? resumeThreshold : (24000 * 0.45);

    this.port.onmessage = (event) => {
      const msg = event.data || {};
      if (msg.type === 'samples' && msg.samples) {
        if (msg.samples instanceof Float32Array) {
          this.queue.push(msg.samples);
        } else if (msg.samples.buffer) {
          this.queue.push(new Float32Array(msg.samples.buffer));
        }
      } else if (msg.type === 'config') {
        if (Number.isFinite(msg.startThreshold)) {
          this.startThreshold = msg.startThreshold;
        }
        if (Number.isFinite(msg.resumeThreshold)) {
          this.resumeThreshold = msg.resumeThreshold;
        }
      } else if (msg.type === 'end') {
        this.ended = true;
      } else if (msg.type === 'reset') {
        this.queue = [];
        this.queueOffset = 0;
        this.hasStarted = false;
        this.isBuffering = true;
        this.ended = false;
        this.firstAudioSent = false;
        this.underruns = 0;
      }
    };
  }

  bufferedSamples() {
    let total = -this.queueOffset;
    for (let i = 0; i < this.queue.length; i++) {
      total += this.queue[i].length;
    }
    return Math.max(0, total);
  }

  fillSilence(channel, fromIdx = 0) {
    for (let i = fromIdx; i < channel.length; i++) {
      channel[i] = 0;
    }
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (!output || output.length === 0) {
      return true;
    }

    const channel = output[0];
    if (!channel) {
      return true;
    }

    const buffered = this.bufferedSamples();

    this.tick += 1;
    if (this.tick % 20 === 0) {
      this.port.postMessage({ type: 'buffer', length: buffered });
    }

    if (!this.hasStarted) {
      if (buffered < this.startThreshold) {
        this.fillSilence(channel);
        if (!this.isBuffering) {
          this.isBuffering = true;
          this.port.postMessage({ type: 'state', state: 'buffering' });
        }
        if (this.ended && buffered === 0) {
          return false;
        }
        return true;
      }
      this.hasStarted = true;
      this.isBuffering = false;
      this.port.postMessage({ type: 'state', state: 'playing' });
    }

    let idx = 0;
    while (idx < channel.length) {
      if (this.queue.length === 0) {
        this.fillSilence(channel, idx);
        if (!this.isBuffering) {
          this.isBuffering = true;
          this.underruns += 1;
          this.port.postMessage({ type: 'underrun', count: this.underruns });
          this.port.postMessage({ type: 'state', state: 'buffering' });
        }
        break;
      }

      const current = this.queue[0];
      const available = current.length - this.queueOffset;
      const toCopy = Math.min(available, channel.length - idx);
      channel.set(current.subarray(this.queueOffset, this.queueOffset + toCopy), idx);
      idx += toCopy;
      this.queueOffset += toCopy;

      if (this.queueOffset >= current.length) {
        this.queue.shift();
        this.queueOffset = 0;
      }
    }

    if (idx === channel.length && this.isBuffering && this.bufferedSamples() >= this.resumeThreshold) {
      this.isBuffering = false;
      this.port.postMessage({ type: 'state', state: 'playing' });
    }

    if (!this.firstAudioSent) {
      for (let i = 0; i < channel.length; i++) {
        if (Math.abs(channel[i]) > 1e-5) {
          this.firstAudioSent = true;
          this.port.postMessage({ type: 'first_audio' });
          break;
        }
      }
    }

    if (this.ended && this.bufferedSamples() === 0) {
      return false;
    }

    return true;
  }
}

registerProcessor('pcm-processor', PCMProcessor);
`;

const encoder = new TextEncoder();

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

const float32ToPcm16 = (samples: Float32Array): Uint8Array => {
  const out = new Uint8Array(samples.length * 2);
  const view = new DataView(out.buffer);
  for (let i = 0; i < samples.length; i++) {
    const clamped = Math.max(-1, Math.min(1, samples[i]));
    const int16 = clamped < 0 ? Math.round(clamped * 32768) : Math.round(clamped * 32767);
    view.setInt16(i * 2, int16, true);
  }
  return out;
};

const pcm16ToFloat32 = (pcm: Uint8Array): Float32Array => {
  const view = new DataView(pcm.buffer, pcm.byteOffset, pcm.byteLength);
  const numSamples = pcm.byteLength / 2;
  const out = new Float32Array(numSamples);
  for (let i = 0; i < numSamples; i++) {
    out[i] = view.getInt16(i * 2, true) / 32768;
  }
  return out;
};

const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    const sub = bytes.subarray(i, i + chunk);
    binary += String.fromCharCode(...sub);
  }
  return btoa(binary);
};

const createWavBlob = (chunks: Uint8Array[], sampleRate: number): Blob => {
  const totalLen = chunks.reduce((acc, c) => acc + c.length, 0);
  const wav = new Uint8Array(44 + totalLen);
  const view = new DataView(wav.buffer);

  const writeString = (offset: number, s: string) => {
    for (let i = 0; i < s.length; i++) {
      view.setUint8(offset + i, s.charCodeAt(i));
    }
  };

  writeString(0, "RIFF");
  view.setUint32(4, 36 + totalLen, true);
  writeString(8, "WAVE");
  writeString(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeString(36, "data");
  view.setUint32(40, totalLen, true);

  let offset = 44;
  for (const chunk of chunks) {
    wav.set(chunk, offset);
    offset += chunk.length;
  }

  return new Blob([wav], { type: "audio/wav" });
};

const toApiError = async (response: Response, fallback: string) => {
  try {
    const data = await response.json() as { error?: string };
    return data.error || fallback;
  } catch {
    return fallback;
  }
};

const isPresetVoice = (voice: string): voice is (typeof PRESET_VOICES)[number] => {
  return (PRESET_VOICES as readonly string[]).includes(voice);
};

export type StreamState = "idle" | "connecting" | "buffering" | "playing" | "finished" | "error";

export interface LatencyMetrics {
  ttfcMs: number | null;
  ttfaMs: number | null;
  totalMs: number | null;
}

export interface PlaybackStats {
  rebufferCount: number;
  startThresholdSec: number;
  resumeThresholdSec: number;
  workerComputeMs: number | null;
  workerMergedChunks: number | null;
}

export interface WasmLoadStatus {
  phase:
    | "idle"
    | "initializing-runtime"
    | "loading-assets"
    | "compiling-model"
    | "ready"
    | "error";
  progress: number;
  message: string;
  source: "local" | "hf" | "manual" | null;
  ready: boolean;
  error: string | null;
}

export interface VoicePreparationInput {
  presetVoice: string;
  customVoiceSpec: string;
  cloneWavBytes: Uint8Array | null;
  embeddingBytes: Uint8Array | null;
  hfRepo: string;
  hfToken: string;
}

export interface WasmInitInput {
  hfRepo: string;
  hfToken: string;
  manualAssets?: {
    configBytes?: Uint8Array;
    weightsBytes?: Uint8Array;
    tokenizerBytes?: Uint8Array;
  };
}

interface GenerationInput {
  text: string;
  voiceSpec?: string;
}

interface StreamHandlers {
  onFirstChunk: () => void;
  onChunk: (samples: Float32Array, pcmChunk: Uint8Array) => void;
  onDone: () => void;
  onWorkerStats?: (stats: { computeMs: number | null; mergedChunks: number | null }) => void;
}

interface RuntimeAdapter {
  mode: UiMode;
  getSampleRate(): number;
  generateStream(input: GenerationInput, handlers: StreamHandlers): Promise<void>;
  stop(): void;
  dispose?(): void;
}

const toWasmLoadStatus = (status: WasmWorkerStatus): WasmLoadStatus => ({
  phase: status.phase,
  progress: status.progress,
  message: status.message,
  source: status.source,
  ready: status.ready,
  error: status.error,
});

class ServerAdapter implements RuntimeAdapter {
  mode: UiMode = "standard";
  private abortController: AbortController | null = null;
  private readonly streamUrl: string;

  constructor(apiBase: string) {
    const base = apiBase.trim().replace(/\/+$/, "");
    this.streamUrl = `${base}/stream`;
  }

  getSampleRate(): number {
    return SAMPLE_RATE;
  }

  stop() {
    this.abortController?.abort();
    this.abortController = null;
  }

  async generateStream(input: GenerationInput, handlers: StreamHandlers): Promise<void> {
    this.stop();
    this.abortController = new AbortController();

    const response = await fetch(this.streamUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        text: input.text,
        voice: input.voiceSpec,
      }),
      signal: this.abortController.signal,
    });

    if (!response.ok) {
      throw new Error(await toApiError(response, "Failed to start stream"));
    }

    const reader = response.body?.getReader();
    if (!reader) {
      throw new Error("No readable stream body from server");
    }

    let seenFirstChunk = false;
    let leftover = new Uint8Array(0);

    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      if (!value || value.byteLength === 0) {
        continue;
      }

      const combined = new Uint8Array(leftover.length + value.length);
      combined.set(leftover);
      combined.set(value, leftover.length);

      const validLen = combined.length - (combined.length % 2);
      const chunk = combined.slice(0, validLen);
      leftover = combined.slice(validLen);

      if (chunk.byteLength === 0) {
        continue;
      }

      if (!seenFirstChunk) {
        seenFirstChunk = true;
        handlers.onFirstChunk();
      }

      handlers.onChunk(pcm16ToFloat32(chunk), chunk);
    }

    handlers.onDone();
    this.abortController = null;
  }
}

class WasmAdapter implements RuntimeAdapter {
  mode: UiMode = "wasm-experimental";
  private readonly wasmBase: string;
  private readonly worker: Worker;
  private ready = false;
  private sampleRate = SAMPLE_RATE;
  private nextRequestId = 1;
  private activeStreamRequestId: number | null = null;
  private statusListener: ((status: WasmLoadStatus) => void) | null = null;
  private streamHandlers: StreamHandlers | null = null;
  private readonly pending = new Map<
    number,
    {
      resolve: (payload: { sampleRate?: number } | undefined) => void;
      reject: (error: Error) => void;
    }
  >();

  constructor(wasmBase: string) {
    this.wasmBase = wasmBase;
    this.worker = new Worker(
      new URL("../workers/wasm-tts.worker.ts", import.meta.url),
      { type: "module" },
    );
    this.worker.onmessage = this.handleWorkerMessage;
  }

  getSampleRate(): number {
    return this.sampleRate;
  }

  stop() {
    const stopMessage: WasmWorkerRequest = { kind: "stop" };
    this.worker.postMessage(stopMessage);
    if (this.activeStreamRequestId != null) {
      const pending = this.pending.get(this.activeStreamRequestId);
      if (pending) {
        pending.reject(new Error("abort"));
        this.pending.delete(this.activeStreamRequestId);
      }
      this.activeStreamRequestId = null;
    }
  }

  dispose() {
    this.stop();
    for (const [requestId, pending] of this.pending) {
      pending.reject(new Error("WASM worker disposed"));
      this.pending.delete(requestId);
    }
    this.worker.terminate();
  }

  isReady(): boolean {
    return this.ready;
  }

  async init(
    input: WasmInitInput,
    onStatus: (next: WasmLoadStatus) => void,
  ): Promise<void> {
    this.stop();
    this.ready = false;
    this.statusListener = onStatus;

    const result = await this.sendRpc({
      kind: "init",
      wasmBase: this.wasmBase,
      hfRepo: input.hfRepo,
      hfToken: input.hfToken,
      manualAssets: input.manualAssets,
    });

    if (typeof result?.sampleRate === "number" && result.sampleRate > 0) {
      this.sampleRate = Math.round(result.sampleRate);
    }
    this.ready = true;
  }

  async loadPresetVoice(voice: string, hfRepo: string, hfToken: string): Promise<void> {
    if (!this.ready) {
      throw new Error("WASM model is not initialized yet.");
    }
    if (!isPresetVoice(voice)) {
      throw new Error(`Unknown preset voice: ${voice}`);
    }

    await this.sendRpc({
      kind: "prepare_voice",
      voice: {
        kind: "preset",
        voice,
        hfRepo,
        hfToken,
      },
    });
  }

  async loadVoiceFromWav(wavBytes: Uint8Array): Promise<void> {
    if (!this.ready) {
      throw new Error("WASM model is not initialized yet.");
    }
    await this.sendRpc({
      kind: "prepare_voice",
      voice: {
        kind: "wav",
        wavBytes,
      },
    });
  }

  async loadVoiceEmbedding(bytes: Uint8Array): Promise<void> {
    if (!this.ready) {
      throw new Error("WASM model is not initialized yet.");
    }
    await this.sendRpc({
      kind: "prepare_voice",
      voice: {
        kind: "embedding",
        embeddingBytes: bytes,
      },
    });
  }

  async generateStream(input: GenerationInput, handlers: StreamHandlers): Promise<void> {
    if (!this.ready) {
      throw new Error("WASM model is not initialized yet.");
    }
    // Voice is prepared via dedicated messages before stream starts.
    void input.voiceSpec;

    this.streamHandlers = handlers;
    try {
      await this.sendRpc({
        kind: "start_stream",
        text: input.text,
      });
    } finally {
      this.streamHandlers = null;
    }
  }

  private sendRpc(
    message:
      | Omit<Extract<WasmWorkerRequest, { kind: "init" }>, "requestId">
      | Omit<Extract<WasmWorkerRequest, { kind: "prepare_voice" }>, "requestId">
      | Omit<Extract<WasmWorkerRequest, { kind: "start_stream" }>, "requestId">,
  ): Promise<{ sampleRate?: number } | undefined> {
    const requestId = this.nextRequestId++;
    const requestWithId = { ...message, requestId } as WasmWorkerRequest;

    if (message.kind === "start_stream") {
      this.activeStreamRequestId = requestId;
    }

    return new Promise((resolve, reject) => {
      this.pending.set(requestId, { resolve, reject });
      this.worker.postMessage(requestWithId);
    });
  }

  private handleWorkerMessage = (event: MessageEvent<WasmWorkerEvent>) => {
    const data = event.data;

    if (data.kind === "status") {
      this.statusListener?.(toWasmLoadStatus(data.status));
      return;
    }

    if (data.kind === "stream_first_chunk") {
      this.streamHandlers?.onFirstChunk();
      return;
    }

    if (data.kind === "stream_chunk") {
      const samples = data.chunk;
      this.streamHandlers?.onWorkerStats?.({
        computeMs: data.computeMs,
        mergedChunks: data.mergedChunks,
      });
      this.streamHandlers?.onChunk(samples, float32ToPcm16(samples));
      return;
    }

    if (data.kind === "stream_done") {
      this.streamHandlers?.onDone();
      return;
    }

    if (data.kind === "stream_error") {
      // The paired rpc_err handles rejection, this is just extra visibility.
      return;
    }

    if (data.kind === "rpc_ok") {
      const pending = this.pending.get(data.requestId);
      if (pending) {
        pending.resolve(data.payload);
        this.pending.delete(data.requestId);
      }
      if (this.activeStreamRequestId === data.requestId) {
        this.activeStreamRequestId = null;
      }
      return;
    }

    if (data.kind === "rpc_err") {
      const pending = this.pending.get(data.requestId);
      if (pending) {
        pending.reject(new Error(data.error));
        this.pending.delete(data.requestId);
      }
      if (this.activeStreamRequestId === data.requestId) {
        this.activeStreamRequestId = null;
      }
    }
  };
}

const initialWasmLoadStatus: WasmLoadStatus = {
  phase: "idle",
  progress: 0,
  message: "WASM engine has not been initialized yet.",
  source: null,
  ready: false,
  error: null,
};

export function useTTSEngine() {
  const bootstrap = useMemo(() => getBootstrapConfig(), []);

  const [mode] = useState<UiMode>(bootstrap.uiMode);
  const [state, setState] = useState<StreamState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [bufferSize, setBufferSize] = useState(0);
  const [generationTime, setGenerationTime] = useState(0);
  const [latency, setLatency] = useState<LatencyMetrics>({
    ttfcMs: null,
    ttfaMs: null,
    totalMs: null,
  });
  const [playbackStats, setPlaybackStats] = useState<PlaybackStats>({
    rebufferCount: 0,
    startThresholdSec: DEFAULT_WASM_START_THRESHOLD_SEC,
    resumeThresholdSec: DEFAULT_WASM_RESUME_THRESHOLD_SEC,
    workerComputeMs: null,
    workerMergedChunks: null,
  });
  const [wasmLoadStatus, setWasmLoadStatus] = useState<WasmLoadStatus>(
    mode === "wasm-experimental"
      ? initialWasmLoadStatus
      : {
          phase: "ready",
          progress: 100,
          message: "Server mode is active.",
          source: null,
          ready: true,
          error: null,
        },
  );

  const audioCtxRef = useRef<AudioContext | null>(null);
  const workletNodeRef = useRef<AudioWorkletNode | null>(null);
  const currentPcmChunksRef = useRef<Uint8Array[]>([]);
  const requestStartRef = useRef<number | null>(null);
  const preparedVoiceSignatureRef = useRef<string>("");
  const latencyRef = useRef<LatencyMetrics>({
    ttfcMs: null,
    ttfaMs: null,
    totalMs: null,
  });
  const rebufferCountRef = useRef(0);
  const wasmStartThresholdSecRef = useRef(DEFAULT_WASM_START_THRESHOLD_SEC);
  const wasmResumeThresholdSecRef = useRef(DEFAULT_WASM_RESUME_THRESHOLD_SEC);

  const adapterRef = useRef<RuntimeAdapter>(
    mode === "wasm-experimental"
      ? new WasmAdapter(bootstrap.wasmBase)
      : new ServerAdapter(bootstrap.apiBase),
  );

  useEffect(() => {
    const adapter = adapterRef.current;
    return () => {
      adapter.stop();
      adapter.dispose?.();
    };
  }, []);

  const setLatencyTracked = useCallback(
    (updater: LatencyMetrics | ((prev: LatencyMetrics) => LatencyMetrics)) => {
      setLatency((prev) => {
        const next = typeof updater === "function"
          ? (updater as (p: LatencyMetrics) => LatencyMetrics)(prev)
          : updater;
        latencyRef.current = next;
        return next;
      });
    },
    [],
  );

  const tuneWasmThresholds = useCallback((ttfaMs: number | null, rebuffers: number) => {
    if (rebuffers > 0) {
      wasmStartThresholdSecRef.current = Math.min(
        1.2,
        wasmStartThresholdSecRef.current + (0.08 * rebuffers),
      );
      wasmResumeThresholdSecRef.current = Math.min(
        0.9,
        wasmResumeThresholdSecRef.current + (0.06 * rebuffers),
      );
    } else if (ttfaMs != null && ttfaMs > 600) {
      wasmStartThresholdSecRef.current = Math.max(
        0.14,
        wasmStartThresholdSecRef.current - 0.03,
      );
      wasmResumeThresholdSecRef.current = Math.max(
        0.24,
        wasmResumeThresholdSecRef.current - 0.01,
      );
    } else {
      wasmStartThresholdSecRef.current = Math.max(
        0.16,
        wasmStartThresholdSecRef.current - 0.01,
      );
      wasmResumeThresholdSecRef.current = Math.max(
        0.26,
        wasmResumeThresholdSecRef.current - 0.005,
      );
    }

    setPlaybackStats((prev) => ({
      ...prev,
      startThresholdSec: wasmStartThresholdSecRef.current,
      resumeThresholdSec: wasmResumeThresholdSecRef.current,
    }));
  }, []);

  const stop = useCallback(() => {
    adapterRef.current.stop();

    if (workletNodeRef.current) {
      workletNodeRef.current.port.postMessage({ type: "reset" });
      workletNodeRef.current.disconnect();
      workletNodeRef.current = null;
    }

    setState("idle");
  }, []);

  const initAudio = useCallback(async (
    sampleRate: number,
    startThresholdSec: number,
    resumeThresholdSec: number,
  ) => {
    const desiredRate = Math.max(8_000, Math.round(sampleRate));

    if (audioCtxRef.current && Math.round(audioCtxRef.current.sampleRate) !== desiredRate) {
      await audioCtxRef.current.close();
      audioCtxRef.current = null;
      workletNodeRef.current = null;
    }

    if (!audioCtxRef.current) {
      const Ctx = (window as typeof window & { webkitAudioContext?: typeof AudioContext }).AudioContext
        || (window as typeof window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctx) {
        throw new Error("AudioContext is not available in this browser.");
      }

      audioCtxRef.current = new Ctx({ sampleRate: desiredRate });

      const blob = new Blob([WORKLET_CODE], { type: "application/javascript" });
      const url = URL.createObjectURL(blob);
      try {
        await audioCtxRef.current.audioWorklet.addModule(url);
      } finally {
        URL.revokeObjectURL(url);
      }
    }

    if (!audioCtxRef.current) {
      throw new Error("Failed to initialize audio context.");
    }

    if (audioCtxRef.current.state === "suspended") {
      await audioCtxRef.current.resume();
    }

    if (workletNodeRef.current) {
      workletNodeRef.current.disconnect();
    }

    workletNodeRef.current = new AudioWorkletNode(audioCtxRef.current, "pcm-processor", {
      processorOptions: {
        startThreshold: Math.round(desiredRate * startThresholdSec),
        resumeThreshold: Math.round(desiredRate * resumeThresholdSec),
      },
    });

    workletNodeRef.current.port.onmessage = (event: MessageEvent) => {
      const data = event.data as {
        type?: string;
        length?: number;
        state?: string;
        count?: number;
      };
      if (data.type === "buffer") {
        setBufferSize(data.length || 0);
      } else if (data.type === "state") {
        if (data.state === "buffering") {
          setState("buffering");
        }
        if (data.state === "playing") {
          setState("playing");
        }
      } else if (data.type === "underrun") {
        const nextCount = typeof data.count === "number"
          ? data.count
          : (rebufferCountRef.current + 1);
        rebufferCountRef.current = nextCount;
        setPlaybackStats((prev) => ({ ...prev, rebufferCount: nextCount }));
      } else if (data.type === "first_audio" && requestStartRef.current != null) {
        const ttfa = performance.now() - requestStartRef.current;
        setLatencyTracked((prev) => ({ ...prev, ttfaMs: ttfa }));
      }
    };

    workletNodeRef.current.connect(audioCtxRef.current.destination);
  }, [setLatencyTracked]);

  const initWasm = useCallback(async (input: WasmInitInput) => {
    if (mode !== "wasm-experimental") {
      return;
    }

    const adapter = adapterRef.current;
    if (!(adapter instanceof WasmAdapter)) {
      return;
    }

    setError(null);

    try {
      const manualAssets = input.manualAssets || {};
      await adapter.init(
        {
          ...input,
          manualAssets: {
            ...manualAssets,
            configBytes: manualAssets.configBytes ?? encoder.encode(DEFAULT_CONFIG_YAML),
          },
        },
        setWasmLoadStatus,
      );
      preparedVoiceSignatureRef.current = "";
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setWasmLoadStatus({
        phase: "error",
        progress: 100,
        message,
        source: null,
        ready: false,
        error: message,
      });
      throw err;
    }
  }, [mode]);

  const prepareVoice = useCallback(async (input: VoicePreparationInput): Promise<string | undefined> => {
    const custom = input.customVoiceSpec.trim();

    if (mode === "standard") {
      if (input.cloneWavBytes && input.cloneWavBytes.byteLength > 0) {
        const b64 = bytesToBase64(input.cloneWavBytes);
        return `data:audio/wav;base64,${b64}`;
      }
      if (custom) {
        return custom;
      }
      return input.presetVoice;
    }

    const adapter = adapterRef.current;
    if (!(adapter instanceof WasmAdapter)) {
      throw new Error("WASM adapter is not active.");
    }

    if (!adapter.isReady()) {
      throw new Error("Initialize the WASM model before generating audio.");
    }

    const voiceSig = [
      input.presetVoice,
      custom,
      input.cloneWavBytes?.byteLength || 0,
      input.embeddingBytes?.byteLength || 0,
      input.hfRepo,
    ].join("|");

    if (preparedVoiceSignatureRef.current === voiceSig) {
      return undefined;
    }

    if (input.cloneWavBytes && input.cloneWavBytes.byteLength > 0) {
      await adapter.loadVoiceFromWav(input.cloneWavBytes);
      preparedVoiceSignatureRef.current = voiceSig;
      return undefined;
    }

    if (input.embeddingBytes && input.embeddingBytes.byteLength > 0) {
      await adapter.loadVoiceEmbedding(input.embeddingBytes);
      preparedVoiceSignatureRef.current = voiceSig;
      return undefined;
    }

    if (custom) {
      throw new Error(
        "Custom voice paths/URLs are not supported in browser WASM mode yet. Use preset voice, WAV clone, or safetensors embedding upload.",
      );
    }

    await adapter.loadPresetVoice(input.presetVoice, input.hfRepo, input.hfToken);
    preparedVoiceSignatureRef.current = voiceSig;
    return undefined;
  }, [mode]);

  const generate = useCallback(async (text: string, voiceSpec?: string) => {
    const normalizedText = text.trim();
    if (!normalizedText) {
      throw new Error("Enter text before generating audio.");
    }

    setError(null);
    setState("connecting");
    setBufferSize(0);
    setGenerationTime(0);
    setLatencyTracked({ ttfcMs: null, ttfaMs: null, totalMs: null });
    currentPcmChunksRef.current = [];
    rebufferCountRef.current = 0;

    const requestStart = performance.now();
    requestStartRef.current = requestStart;

    const adapter = adapterRef.current;
    const sampleRate = adapter.getSampleRate();
    const startThresholdSec = mode === "wasm-experimental"
      ? wasmStartThresholdSecRef.current
      : 2.8;
    const resumeThresholdSec = mode === "wasm-experimental"
      ? wasmResumeThresholdSecRef.current
      : 0.45;
    setPlaybackStats((prev) => ({
      ...prev,
      rebufferCount: 0,
      startThresholdSec,
      resumeThresholdSec,
      workerComputeMs: null,
      workerMergedChunks: null,
    }));

    try {
      await initAudio(sampleRate, startThresholdSec, resumeThresholdSec);

      setState("buffering");
      await adapter.generateStream(
        { text: normalizedText, voiceSpec },
        {
          onFirstChunk: () => {
            if (requestStartRef.current != null) {
              const ttfc = performance.now() - requestStartRef.current;
              setLatencyTracked((prev) => ({ ...prev, ttfcMs: ttfc }));
            }
          },
          onChunk: (samples, pcmChunk) => {
            currentPcmChunksRef.current.push(pcmChunk);
            workletNodeRef.current?.port.postMessage({ type: "samples", samples }, [samples.buffer]);
          },
          onWorkerStats: ({ computeMs, mergedChunks }) => {
            setPlaybackStats((prev) => ({
              ...prev,
              workerComputeMs: computeMs ?? prev.workerComputeMs,
              workerMergedChunks: mergedChunks ?? prev.workerMergedChunks,
            }));
          },
          onDone: () => {
            workletNodeRef.current?.port.postMessage({ type: "end" });
          },
        },
      );

      const totalMs = performance.now() - requestStart;
      setGenerationTime(totalMs / 1000);
      setLatencyTracked((prev) => ({ ...prev, totalMs }));

      if (mode === "wasm-experimental") {
        tuneWasmThresholds(latencyRef.current.ttfaMs, rebufferCountRef.current);
      }

      setState("finished");
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message.includes("abort") || message.includes("Abort")) {
        setState("idle");
        return;
      }
      setError(message);
      setState("error");
      throw err;
    }
  }, [initAudio, mode, setLatencyTracked, tuneWasmThresholds]);

  const downloadWav = useCallback(() => {
    if (currentPcmChunksRef.current.length === 0) {
      return;
    }

    const blob = createWavBlob(currentPcmChunksRef.current, adapterRef.current.getSampleRate());
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "pocket-tts-output.wav";
    a.click();
    URL.revokeObjectURL(url);
  }, []);

  const runTtfaProbe = useCallback(async (text: string, voiceSpec?: string): Promise<LatencyMetrics> => {
    await generate(text, voiceSpec);
    // Give the worklet one event turn to emit first_audio if it is still in-flight.
    await sleep(0);
    return { ...latencyRef.current };
  }, [generate]);

  return useMemo(() => ({
    mode,
    state,
    error,
    bufferSize,
    generationTime,
    latency,
    playbackStats,
    wasmLoadStatus,
    generate,
    stop,
    downloadWav,
    hasAudio: currentPcmChunksRef.current.length > 0,
    initWasm,
    prepareVoice,
    runTtfaProbe,
  }), [
    mode,
    state,
    error,
    bufferSize,
    generationTime,
    latency,
    playbackStats,
    wasmLoadStatus,
    generate,
    stop,
    downloadWav,
    initWasm,
    prepareVoice,
    runTtfaProbe,
  ]);
}

export const useTTSStream = useTTSEngine;
