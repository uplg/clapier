import { useEffect, useMemo, useState } from "react";
import { useTTSEngine, type LatencyMetrics } from "@/hooks/use-tts-stream";
import { VoiceSelector } from "@/components/tts/voice-selector";
import { BufferVisualizer } from "@/components/tts/buffer-visualizer";
import { Card, CardContent, CardDescription, CardHeader, CardTitle, CardFooter } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  PlayIcon,
  SquareIcon,
  DownloadIcon,
  Volume2Icon,
  AlertCircleIcon,
  GithubIcon,
  MessageSquare,
  FlaskConical,
  RefreshCw,
  ChevronDown,
  Sparkles,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";

const HF_REPO_STORAGE_KEY = "pocket-tts.hf-repo";
const HF_TOKEN_STORAGE_KEY = "pocket-tts.hf-token";
const HF_REMEMBER_STORAGE_KEY = "pocket-tts.hf-remember";

const readFileBytes = async (file: File | null): Promise<Uint8Array | null> => {
  if (!file) {
    return null;
  }
  return new Uint8Array(await file.arrayBuffer());
};

const describeVoiceSelection = (
  preset: string,
  custom: string,
  hasWav: boolean,
  hasEmbedding: boolean,
): string => {
  if (hasWav) {
    return "Voice clone from WAV";
  }
  if (hasEmbedding) {
    return "Voice embedding upload";
  }
  if (custom.trim()) {
    return "Custom voice spec";
  }
  return `Preset: ${preset}`;
};

const isLatencyPass = (latency: LatencyMetrics | null) => {
  if (!latency || latency.ttfaMs == null) {
    return false;
  }
  return latency.ttfaMs <= 600;
};

export default function App() {
  const [text, setText] = useState(
    "Hello world! I am Pocket TTS running in Rust. I am blazingly fast on CPU.",
  );
  const [selectedVoice, setSelectedVoice] = useState<string | null>("alba");
  const [customVoice, setCustomVoice] = useState("");
  const [cloneWavFile, setCloneWavFile] = useState<File | null>(null);
  const [embeddingFile, setEmbeddingFile] = useState<File | null>(null);

  const [rememberHf, setRememberHf] = useState(() => {
    return window.localStorage.getItem(HF_REMEMBER_STORAGE_KEY) !== "0";
  });
  const [hfRepo, setHfRepo] = useState(() => {
    return window.localStorage.getItem(HF_REPO_STORAGE_KEY) || "kyutai/pocket-tts";
  });
  const [hfToken, setHfToken] = useState(() => {
    if (window.localStorage.getItem(HF_REMEMBER_STORAGE_KEY) === "0") {
      return "";
    }
    return window.localStorage.getItem(HF_TOKEN_STORAGE_KEY) || "";
  });

  const [manualConfigFile, setManualConfigFile] = useState<File | null>(null);
  const [manualWeightsFile, setManualWeightsFile] = useState<File | null>(null);
  const [manualTokenizerFile, setManualTokenizerFile] = useState<File | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const [probeText, setProbeText] = useState("Hello, this is a latency check.");
  const [probeResult, setProbeResult] = useState<LatencyMetrics | null>(null);

  const {
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
    hasAudio,
    initWasm,
    prepareVoice,
    runTtfaProbe,
  } = useTTSEngine();

  useEffect(() => {
    if (!rememberHf) {
      window.localStorage.setItem(HF_REMEMBER_STORAGE_KEY, "0");
      window.localStorage.removeItem(HF_TOKEN_STORAGE_KEY);
      return;
    }

    window.localStorage.setItem(HF_REMEMBER_STORAGE_KEY, "1");
    window.localStorage.setItem(HF_REPO_STORAGE_KEY, hfRepo);
    window.localStorage.setItem(HF_TOKEN_STORAGE_KEY, hfToken);
  }, [rememberHf, hfRepo, hfToken]);

  const isIdle = state === "idle" || state === "finished" || state === "error";
  const isWasmMode = mode === "wasm-experimental";
  const wasmReady = wasmLoadStatus.ready;
  const voiceStatus = useMemo(
    () =>
      describeVoiceSelection(
        selectedVoice || "alba",
        isWasmMode ? "" : customVoice,
        !!cloneWavFile,
        isWasmMode && !!embeddingFile,
      ),
    [selectedVoice, customVoice, cloneWavFile, embeddingFile, isWasmMode],
  );

  const wasmStatusTone = useMemo(() => {
    if (wasmLoadStatus.phase === "error") {
      return "text-destructive";
    }
    if (wasmLoadStatus.phase === "ready") {
      return "text-green-600";
    }
    return "text-muted-foreground";
  }, [wasmLoadStatus.phase]);

  const handleVoiceSelect = (voice: string | null) => {
    setSelectedVoice(voice);
    if (voice) {
      setCustomVoice("");
    }
  };

  const handleCustomVoiceChange = (voiceSpec: string) => {
    if (isWasmMode) {
      return;
    }
    setCustomVoice(voiceSpec);
    if (voiceSpec.trim()) {
      setSelectedVoice(null);
    }
  };

  const buildVoicePreparationInput = async () => {
    const wavBytes = await readFileBytes(cloneWavFile);
    const embeddingBytes = isWasmMode ? await readFileBytes(embeddingFile) : null;

    return {
      presetVoice: selectedVoice || "alba",
      customVoiceSpec: customVoice,
      cloneWavBytes: wavBytes,
      embeddingBytes,
      hfRepo,
      hfToken,
    };
  };

  const handleGenerate = async () => {
    try {
      const voiceInput = await buildVoicePreparationInput();
      const voiceSpec = await prepareVoice(voiceInput);
      await generate(text, voiceSpec);
    } catch {
      // Hook already exposes the surfaced error state.
    }
  };

  const handleStop = () => {
    stop();
  };

  const handleInitializeWasm = async () => {
    try {
      const [configBytes, weightsBytes, tokenizerBytes] = await Promise.all([
        readFileBytes(manualConfigFile),
        readFileBytes(manualWeightsFile),
        readFileBytes(manualTokenizerFile),
      ]);

      await initWasm({
        hfRepo,
        hfToken,
        manualAssets: {
          configBytes: configBytes || undefined,
          weightsBytes: weightsBytes || undefined,
          tokenizerBytes: tokenizerBytes || undefined,
        },
      });
    } catch {
      // Hook handles error display.
    }
  };

  const handleProbe = async () => {
    try {
      const voiceInput = await buildVoicePreparationInput();
      const voiceSpec = await prepareVoice(voiceInput);
      const result = await runTtfaProbe(probeText, voiceSpec);
      setProbeResult(result);
    } catch {
      // Hook handles error display.
    }
  };

  return (
    <div className="min-h-screen bg-background selection:bg-primary/20 flex flex-col items-center justify-center p-4 md:p-8 font-sans transition-colors duration-500">
      <div className="fixed inset-0 overflow-hidden -z-10 bg-[radial-gradient(circle_at_top_left,var(--color-primary)_0%,transparent_30%),radial-gradient(circle_at_bottom_right,oklch(0.5_0.1_260)_0%,transparent_30%)] opacity-[0.05]" />

      <main className="w-full max-w-3xl animate-in fade-in slide-in-from-bottom-4 duration-1000">
        <div className="flex items-center justify-between mb-8">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-primary/10 rounded-xl border border-primary/20 shadow-inner">
              <Volume2Icon className="w-6 h-6 text-primary" />
            </div>
            <div>
              <h1 className="text-2xl font-bold tracking-tight text-foreground/90 flex items-center gap-2">
                Pocket TTS
                <Badge variant="outline" className="text-[10px] py-0 font-medium border-primary/20 text-primary/70">
                  CANDLE PORT
                </Badge>
              </h1>
              <p className="text-xs text-muted-foreground font-medium flex items-center gap-2">
                Blazingly fast CPU Text-to-Speech
                {isWasmMode && (
                  <Badge variant="outline" className="text-[10px] uppercase border-amber-500/40 text-amber-600">
                    Experimental WASM
                  </Badge>
                )}
              </p>
            </div>
          </div>
          <a
            href="https://github.com/babybirdprd/pocket-tts-candle"
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center justify-center size-9 rounded-full hover:bg-muted/50 transition-colors"
          >
            <GithubIcon className="w-4 h-4" />
          </a>
        </div>

        <div className="grid gap-6">
          <Card className="border-muted-foreground/10 bg-card/50 backdrop-blur-xl shadow-2xl shadow-primary/5 ring-1 ring-white/10 overflow-hidden">
            <CardHeader className="pb-4">
              <div className="space-y-1">
                <CardTitle className="text-lg flex items-center gap-2">
                  <MessageSquare className="w-4 h-4 text-primary/70" />
                  Input Text
                </CardTitle>
                <CardDescription>What should I say?</CardDescription>
              </div>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="relative group">
                <Textarea
                  placeholder="Type something amazing..."
                  className="min-h-[130px] text-base leading-relaxed resize-none bg-muted/20 border-muted-foreground/10 group-hover:border-primary/30 transition-all duration-300 focus-visible:ring-primary/20 focus-visible:ring-offset-0 focus-visible:border-primary/50"
                  value={text}
                  onChange={(e) => setText(e.target.value)}
                />
                <div className="absolute bottom-3 right-3 text-[10px] font-mono text-muted-foreground/50 opacity-0 group-focus-within:opacity-100 transition-opacity">
                  {text.length} characters
                </div>
              </div>

              <VoiceSelector
                selectedVoice={selectedVoice}
                customVoice={customVoice}
                onVoiceSelect={handleVoiceSelect}
                onCustomVoiceChange={handleCustomVoiceChange}
                customEnabled={!isWasmMode}
                customLabel={
                  isWasmMode
                    ? "Custom URL/path is disabled in WASM mode"
                    : "Or use a custom URL / Path"
                }
                customPlaceholder={
                  isWasmMode
                    ? "Use preset, WAV clone, or embedding upload"
                    : "hf://kyutai/tts-voices/voice.wav"
                }
              />

              <div className="grid md:grid-cols-2 gap-4 rounded-lg border border-muted-foreground/10 p-4 bg-muted/10">
                <div className="space-y-2">
                  <Label htmlFor="clone-wav" className="text-xs uppercase tracking-wide text-muted-foreground">
                    Clone Voice WAV
                  </Label>
                  <Input
                    id="clone-wav"
                    type="file"
                    accept=".wav"
                    onChange={(e) => setCloneWavFile(e.target.files?.[0] || null)}
                  />
                  <p className="text-[10px] text-muted-foreground">
                    Optional 5-10s WAV sample for voice cloning.
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="clone-embed" className="text-xs uppercase tracking-wide text-muted-foreground">
                    Embedding (.safetensors)
                  </Label>
                  <Input
                    id="clone-embed"
                    type="file"
                    accept=".safetensors"
                    disabled={!isWasmMode}
                    onChange={(e) => setEmbeddingFile(e.target.files?.[0] || null)}
                  />
                  <p className="text-[10px] text-muted-foreground">
                    {isWasmMode
                      ? "Optional precomputed embedding for WASM mode."
                      : "Available in experimental WASM mode only."}
                  </p>
                </div>
              </div>

              <div className="rounded-lg border border-muted-foreground/10 px-3 py-2 text-xs bg-muted/10">
                <span className="font-semibold text-muted-foreground">Active voice:</span> {voiceStatus}
              </div>

              {isWasmMode && (
                <div className="space-y-4 rounded-lg border border-amber-500/20 bg-amber-500/5 p-4">
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <h3 className="text-sm font-semibold flex items-center gap-2">
                        <FlaskConical className="w-4 h-4 text-amber-600" />
                        WASM Engine Setup
                      </h3>
                      <p className="text-xs text-muted-foreground mt-1">
                        Experimental browser-side inference. Initialize once before generating.
                      </p>
                    </div>
                    <Button
                      variant={wasmReady ? "outline" : "default"}
                      size="sm"
                      onClick={handleInitializeWasm}
                      className="shrink-0"
                    >
                      {wasmReady ? (
                        <>
                          <RefreshCw className="w-3 h-3" data-icon="inline-start" />
                          Re-initialize
                        </>
                      ) : (
                        <>
                          <Sparkles className="w-3 h-3" data-icon="inline-start" />
                          Initialize
                        </>
                      )}
                    </Button>
                  </div>

                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-[11px]">
                      <span className={wasmStatusTone}>{wasmLoadStatus.message}</span>
                      <span className="font-mono text-muted-foreground">{Math.round(wasmLoadStatus.progress)}%</span>
                    </div>
                    <Progress value={wasmLoadStatus.progress} className="h-2" />
                    <div className="text-[10px] text-muted-foreground">
                      Source: {wasmLoadStatus.source || "n/a"} | Phase: {wasmLoadStatus.phase}
                    </div>
                  </div>

                  <div className="grid md:grid-cols-2 gap-3">
                    <div className="space-y-2">
                      <Label htmlFor="hf-repo" className="text-xs">HF Repository</Label>
                      <Input
                        id="hf-repo"
                        value={hfRepo}
                        onChange={(e) => setHfRepo(e.target.value)}
                        placeholder="kyutai/pocket-tts"
                      />
                    </div>
                    <div className="space-y-2">
                      <Label htmlFor="hf-token" className="text-xs">HF Token (for gated assets)</Label>
                      <Input
                        id="hf-token"
                        value={hfToken}
                        onChange={(e) => setHfToken(e.target.value)}
                        type="password"
                        placeholder="hf_..."
                      />
                    </div>
                  </div>

                  <label className="flex items-center gap-2 text-xs text-muted-foreground select-none">
                    <input
                      type="checkbox"
                      checked={rememberHf}
                      onChange={(e) => setRememberHf(e.target.checked)}
                    />
                    Remember HF repo/token on this browser
                  </label>

                  <div className="rounded-md border border-muted-foreground/10 bg-background/60 overflow-hidden">
                    <button
                      type="button"
                      className="w-full px-3 py-2 text-left text-xs font-medium flex items-center justify-between hover:bg-muted/30 transition-colors"
                      onClick={() => setShowAdvanced((prev) => !prev)}
                    >
                      <span>Advanced manual asset overrides</span>
                      <ChevronDown className={`w-3 h-3 transition-transform ${showAdvanced ? "rotate-180" : ""}`} />
                    </button>
                    {showAdvanced && (
                      <div className="p-3 grid md:grid-cols-3 gap-3 border-t border-muted-foreground/10">
                        <div className="space-y-1">
                          <Label htmlFor="manual-config" className="text-[10px] text-muted-foreground">Config YAML</Label>
                          <Input
                            id="manual-config"
                            type="file"
                            accept=".yaml,.yml"
                            onChange={(e) => setManualConfigFile(e.target.files?.[0] || null)}
                          />
                        </div>
                        <div className="space-y-1">
                          <Label htmlFor="manual-weights" className="text-[10px] text-muted-foreground">Model Weights</Label>
                          <Input
                            id="manual-weights"
                            type="file"
                            accept=".safetensors"
                            onChange={(e) => setManualWeightsFile(e.target.files?.[0] || null)}
                          />
                        </div>
                        <div className="space-y-1">
                          <Label htmlFor="manual-tokenizer" className="text-[10px] text-muted-foreground">Tokenizer JSON</Label>
                          <Input
                            id="manual-tokenizer"
                            type="file"
                            accept=".json"
                            onChange={(e) => setManualTokenizerFile(e.target.files?.[0] || null)}
                          />
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}

              <BufferVisualizer
                state={state}
                bufferSize={bufferSize}
                generationTime={generationTime}
                latency={latency}
                playbackStats={playbackStats}
              />

              {error && (
                <Alert variant="destructive" className="animate-in fade-in zoom-in-95 duration-300">
                  <AlertCircleIcon className="h-4 w-4" />
                  <AlertTitle>Generation Error</AlertTitle>
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}
            </CardContent>
            <CardFooter className="bg-muted/5 border-t border-muted-foreground/5 py-4 flex flex-col gap-3">
              <div className="flex gap-2 w-full">
                {isIdle ? (
                  <Button
                    className="flex-1 h-12 text-base font-semibold transition-all duration-300 shadow-lg shadow-primary/25 hover:shadow-primary/40 group active:scale-[0.98]"
                    onClick={handleGenerate}
                    disabled={isWasmMode && !wasmReady}
                  >
                    <PlayIcon className="w-4 h-4 transition-transform group-hover:scale-110" data-icon="inline-start" />
                    Generate Audio
                  </Button>
                ) : (
                  <Button
                    variant="destructive"
                    className="flex-1 h-12 text-base font-semibold group active:scale-[0.98]"
                    onClick={handleStop}
                  >
                    <SquareIcon className="w-4 h-4 group-hover:scale-110" data-icon="inline-start" />
                    {state === "buffering" ? "Cancel Buffering" : "Stop Playback"}
                  </Button>
                )}

                <Button
                  variant="outline"
                  className="h-12 w-12 p-0 border-muted-foreground/10 hover:bg-primary/5 transition-all duration-300 active:scale-[0.98]"
                  disabled={!hasAudio}
                  onClick={downloadWav}
                  title="Download WAV"
                >
                  <DownloadIcon className="w-5 h-5 text-muted-foreground group-hover:text-primary transition-colors" />
                </Button>
              </div>
            </CardFooter>
          </Card>

          <Card className="border-muted-foreground/10 bg-card/40 backdrop-blur-xl">
            <CardHeader className="pb-3">
              <CardTitle className="text-sm flex items-center gap-2">
                <FlaskConical className="w-4 h-4 text-primary/70" />
                Manual TTFA Verification
              </CardTitle>
              <CardDescription>
                Measure Time-To-First-Audio for the 600ms target requested by maintainers.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              <Textarea
                className="min-h-[80px] text-sm bg-muted/20"
                value={probeText}
                onChange={(e) => setProbeText(e.target.value)}
              />
              <div className="flex gap-2">
                <Button variant="outline" onClick={handleProbe} disabled={isWasmMode && !wasmReady}>
                  Run TTFA Probe
                </Button>
                {probeResult && (
                  <Badge
                    variant="outline"
                    className={isLatencyPass(probeResult) ? "border-green-600/40 text-green-700" : "border-red-600/40 text-red-700"}
                  >
                    {isLatencyPass(probeResult) ? "PASS <= 600ms" : "FAIL > 600ms"}
                  </Badge>
                )}
              </div>
              <div className="grid grid-cols-3 gap-2 text-xs">
                <div className="rounded border border-muted-foreground/10 p-2">
                  <div className="text-muted-foreground">TTFC</div>
                  <div className="font-mono mt-1">{probeResult?.ttfcMs != null ? `${probeResult.ttfcMs.toFixed(0)}ms` : "-"}</div>
                </div>
                <div className="rounded border border-muted-foreground/10 p-2">
                  <div className="text-muted-foreground">TTFA</div>
                  <div className="font-mono mt-1">{probeResult?.ttfaMs != null ? `${probeResult.ttfaMs.toFixed(0)}ms` : "-"}</div>
                </div>
                <div className="rounded border border-muted-foreground/10 p-2">
                  <div className="text-muted-foreground">Total</div>
                  <div className="font-mono mt-1">{probeResult?.totalMs != null ? `${probeResult.totalMs.toFixed(0)}ms` : "-"}</div>
                </div>
              </div>
            </CardContent>
          </Card>
        </div>

        <footer className="mt-12 text-center space-y-4">
          <p className="text-[10px] text-muted-foreground uppercase tracking-[0.2em] font-bold">
            Powered by Candle & PyO3 • Zero Python Runtime
          </p>
        </footer>
      </main>
    </div>
  );
}
