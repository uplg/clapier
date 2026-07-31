export type UiMode = "standard" | "wasm-experimental";

export interface BootstrapConfig {
  uiMode: UiMode;
  apiBase: string;
  wasmBase: string;
}

type RawBootstrap = {
  uiMode?: string;
  apiBase?: string;
  wasmBase?: string;
  ui_mode?: string;
  api_base?: string;
  wasm_base?: string;
};

declare global {
  interface Window {
    __POCKET_TTS_BOOTSTRAP__?: RawBootstrap;
  }
}

const normalizeMode = (raw: string | undefined): UiMode => {
  return raw === "wasm-experimental" ? "wasm-experimental" : "standard";
};

export const getBootstrapConfig = (): BootstrapConfig => {
  const raw = window.__POCKET_TTS_BOOTSTRAP__;

  const mode = normalizeMode(raw?.uiMode || raw?.ui_mode);
  const apiBase = (raw?.apiBase || raw?.api_base || "").trim();
  const wasmBase = (raw?.wasmBase || raw?.wasm_base || "/wasm/pkg").trim() || "/wasm/pkg";

  return {
    uiMode: mode,
    apiBase,
    wasmBase,
  };
};
