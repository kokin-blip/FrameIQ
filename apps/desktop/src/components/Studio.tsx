import { useRef, useState } from "react";
import { Button } from "@frameiq/ui";
import { FileVideo } from "lucide-react";
import { api, timestamp } from "../client";
import type { Workspace } from "../workspace/useWorkspace";
import { StudioToolbar } from "./StudioToolbar";
import { Timeline } from "./Timeline";
import { PresetPanel } from "./PresetPanel";
import { AnalysisPanel } from "./AnalysisPanel";
import { FrameComparison } from "./FrameComparison";
import type { Selection, QualityMetricsV1 } from "../contracts";
import { FrameCollection } from "./FrameCollection";

export function Studio({
  w,
  images,
}: {
  w: Workspace;
  images: Record<string, string>;
}) {
  const [precision, setPrecision] = useState(false);
  const [panel, setPanel] = useState("selections");
  const [comparisonMetrics, setComparisonMetrics] = useState<
    Record<string, QualityMetricsV1>
  >({});
  const [comparison, setComparison] = useState<Selection[]>([]);
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(
    null,
  );
  return (
    <div className="studio">
      <StudioToolbar w={w} />
      <div className="editor-layout">
        <div className="viewer-column">
          {comparison.length > 0 ? (
            <FrameComparison
              frames={comparison}
              metrics={comparisonMetrics}
              onClose={() => setComparison([])}
              onError={w.setError}
            />
          ) : (
            <section
              className={
                w.dropActive ? "canvas-stage drop-active" : "canvas-stage"
              }
              aria-label="Video frame canvas"
            >
              <div
                className={`image-viewport ${w.zoom > 1 ? "can-pan" : ""}`}
                onPointerDown={(e) => {
                  if (w.zoom <= 1) return;
                  drag.current = {
                    x: e.clientX,
                    y: e.clientY,
                    px: w.pan.x,
                    py: w.pan.y,
                  };
                  e.currentTarget.setPointerCapture(e.pointerId);
                }}
                onPointerMove={(e) => {
                  if (!drag.current) return;
                  w.setPan({
                    x: drag.current.px + e.clientX - drag.current.x,
                    y: drag.current.py + e.clientY - drag.current.y,
                  });
                }}
                onPointerUp={() => {
                  drag.current = null;
                }}
                onPointerCancel={() => {
                  drag.current = null;
                }}
              >
                {w.image ? (
                  <img
                    src={api.image(w.image.imagePath)}
                    alt={`Frame ${w.image.frameRef.presentationOrdinal} at ${timestamp(w.image.timeSeconds)}`}
                    style={{
                      transform: `translate(${w.pan.x}px,${w.pan.y}px) scale(${w.zoom})`,
                    }}
                    draggable={false}
                  />
                ) : (
                  <button
                    className="attach-zone"
                    aria-label="Attach video"
                    onClick={() => void w.importVideo()}
                  >
                    <FileVideo size={32} />
                    <h2>Drop a video here</h2>
                    <p>or click to attach</p>
                  </button>
                )}
              </div>
              {w.busy && (
                <div className="viewer-status" role="status">
                  Resolving frame…
                </div>
              )}
            </section>
          )}
          {w.activeSource?.error && (
            <div className="source-recovery" role="status">
              <span>{w.activeSource.error.message}</span>
              <Button
                disabled={w.activeJobs.some((j) => j.stage === "relocating")}
                onClick={() => void w.locateSource(w.activeSource!.id)}
              >
                Locate file
              </Button>
              <Button onClick={() => void w.importVideo()}>
                Import replacement
              </Button>
            </div>
          )}
          <Timeline
            w={w}
            expanded={precision}
            onToggle={() => setPrecision((v) => !v)}
          />
        </div>
        <div className="editor-dock">
          <div className="dock-tabs" aria-label="Editor panels">
            <Button
              aria-pressed={panel === "selections"}
              onClick={() => setPanel("selections")}
            >
              Selections
            </Button>
            <Button
              aria-pressed={panel === "analysis"}
              onClick={() => setPanel("analysis")}
            >
              Analysis
            </Button>
            <Button
              aria-pressed={panel === "presets"}
              onClick={() => setPanel("presets")}
            >
              Presets
            </Button>
          </div>
          {panel === "selections" ? (
            <FrameCollection w={w} images={images} />
          ) : panel === "analysis" ? (
            <AnalysisPanel
              w={w}
              onCompare={(frames, metrics) => {
                setComparison(frames);
                setComparisonMetrics(metrics);
              }}
            />
          ) : (
            <PresetPanel w={w} />
          )}
        </div>
      </div>
    </div>
  );
}
