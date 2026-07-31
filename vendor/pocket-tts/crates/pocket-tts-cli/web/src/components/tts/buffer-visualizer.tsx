import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import type { StreamState, LatencyMetrics, PlaybackStats } from "@/hooks/use-tts-stream";
import { Activity, Play, Zap, Info, Gauge } from "lucide-react";

interface BufferVisualizerProps {
  state: StreamState;
  bufferSize: number;
  generationTime: number;
  latency: LatencyMetrics;
  playbackStats: PlaybackStats;
}

export function BufferVisualizer({ state, bufferSize, generationTime, latency, playbackStats }: BufferVisualizerProps) {
  const maxSamples = 24000 * 5;
  const percentage = Math.min((bufferSize / maxSamples) * 100, 100);
  const bufferSecs = (bufferSize / 24000).toFixed(1);

  if (state === "idle" && generationTime === 0 && latency.ttfaMs == null && latency.ttfcMs == null) {
    return null;
  }

  return (
    <div className="space-y-3 pt-2">
      <div className="flex items-center justify-between text-xs">
        <div className="flex items-center gap-2 text-muted-foreground">
          <Activity className="w-3 h-3" />
          <span>Buffer Health</span>
        </div>
        <div className="font-mono text-primary flex items-center gap-2">
          {state === "playing" && <Play className="w-3 h-3 animate-pulse" />}
          {state === "buffering" && <Activity className="w-3 h-3 animate-bounce" />}
          <span>{bufferSecs}s</span>
        </div>
      </div>

      <Progress value={percentage} className="h-2 bg-muted-foreground/10" />

      <div className="flex items-center justify-between pt-1">
        <div className="flex gap-2">
          {state === "playing" && (
            <Badge variant="outline" className="text-[10px] uppercase border-primary/30 text-primary bg-primary/5">
              Playing
            </Badge>
          )}
          {state === "buffering" && (
            <Badge variant="outline" className="text-[10px] uppercase border-amber-500/30 text-amber-500 bg-amber-500/5">
              Smart Buffering...
            </Badge>
          )}
          {state === "finished" && (
            <Badge variant="outline" className="text-[10px] uppercase border-green-500/30 text-green-500 bg-green-500/5">
              Ready
            </Badge>
          )}
        </div>

        {generationTime > 0 && (
          <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
            <Zap className="w-3 h-3 text-amber-500" />
            <span>Generated in {generationTime.toFixed(2)}s</span>
          </div>
        )}
      </div>

      {(latency.ttfcMs != null || latency.ttfaMs != null) && (
        <div className="grid grid-cols-2 gap-2 text-[10px]">
          <div className="rounded-md border border-muted-foreground/10 bg-muted/20 p-2">
            <div className="flex items-center gap-1 text-muted-foreground">
              <Gauge className="w-3 h-3" />
              TTFC
            </div>
            <div className="font-mono text-foreground mt-1">{latency.ttfcMs != null ? `${latency.ttfcMs.toFixed(0)}ms` : "-"}</div>
          </div>
          <div className="rounded-md border border-muted-foreground/10 bg-muted/20 p-2">
            <div className="flex items-center gap-1 text-muted-foreground">
              <Gauge className="w-3 h-3" />
              TTFA
            </div>
            <div className="font-mono text-foreground mt-1">{latency.ttfaMs != null ? `${latency.ttfaMs.toFixed(0)}ms` : "-"}</div>
          </div>
        </div>
      )}

      {state === "buffering" && (
        <div className="flex items-center gap-2 text-[10px] text-muted-foreground bg-muted/30 p-2 rounded-md border border-muted-foreground/10">
          <Info className="w-3 h-3" />
          <span>Optimizing playback latency based on generation speed.</span>
        </div>
      )}

      <div className="grid grid-cols-2 gap-2 text-[10px]">
        <div className="rounded-md border border-muted-foreground/10 bg-muted/20 p-2">
          <div className="text-muted-foreground">Rebuffers</div>
          <div className="font-mono text-foreground mt-1">{playbackStats.rebufferCount}</div>
        </div>
        <div className="rounded-md border border-muted-foreground/10 bg-muted/20 p-2">
          <div className="text-muted-foreground">Thresholds</div>
          <div className="font-mono text-foreground mt-1">
            {playbackStats.startThresholdSec.toFixed(2)}s / {playbackStats.resumeThresholdSec.toFixed(2)}s
          </div>
        </div>
      </div>
    </div>
  );
}
