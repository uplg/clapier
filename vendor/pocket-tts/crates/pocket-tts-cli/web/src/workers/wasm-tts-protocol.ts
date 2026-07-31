export type WasmAssetSource = "local" | "hf" | "manual" | null;

export type WasmLoadPhase =
  | "idle"
  | "initializing-runtime"
  | "loading-assets"
  | "compiling-model"
  | "ready"
  | "error";

export interface WasmWorkerStatus {
  phase: WasmLoadPhase;
  progress: number;
  message: string;
  source: WasmAssetSource;
  ready: boolean;
  error: string | null;
}

export interface WasmWorkerManualAssets {
  configBytes?: Uint8Array;
  weightsBytes?: Uint8Array;
  tokenizerBytes?: Uint8Array;
}

export type WasmWorkerVoiceInput =
  | {
      kind: "preset";
      voice: string;
      hfRepo: string;
      hfToken: string;
    }
  | {
      kind: "wav";
      wavBytes: Uint8Array;
    }
  | {
      kind: "embedding";
      embeddingBytes: Uint8Array;
    };

export type WasmWorkerRequest =
  | {
      kind: "init";
      requestId: number;
      wasmBase: string;
      hfRepo: string;
      hfToken: string;
      manualAssets?: WasmWorkerManualAssets;
    }
  | {
      kind: "prepare_voice";
      requestId: number;
      voice: WasmWorkerVoiceInput;
    }
  | {
      kind: "start_stream";
      requestId: number;
      text: string;
    }
  | {
      kind: "stop";
    };

export type WasmWorkerEvent =
  | {
      kind: "status";
      status: WasmWorkerStatus;
    }
  | {
      kind: "rpc_ok";
      requestId: number;
      payload?: {
        sampleRate?: number;
      };
    }
  | {
      kind: "rpc_err";
      requestId: number;
      error: string;
    }
  | {
      kind: "stream_first_chunk";
    }
  | {
      kind: "stream_chunk";
      chunk: Float32Array;
      computeMs: number | null;
      mergedChunks: number | null;
    }
  | {
      kind: "stream_done";
    }
  | {
      kind: "stream_error";
      error: string;
    };

