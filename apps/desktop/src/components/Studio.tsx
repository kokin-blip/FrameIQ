import { RegionEditor } from "./RegionEditor";
import { CloudAnalysisPanel } from "./CloudAnalysisPanel";
import { useRef, useState, useEffect } from "react";
import { Button } from "@frameiq/ui";
import { FileVideo } from "lucide-react";
import { api, timestamp } from "../client";
import type { Workspace } from "../workspace/useWorkspace";
import { StudioToolbar } from "./StudioToolbar";
import { Timeline } from "./Timeline";
import { AnalysisPanel } from "./AnalysisPanel";
import { FrameComparison } from "./FrameComparison";
import type { Selection, QualityMetricsV1, RegionRectV1 } from "../contracts";
import { FrameCollection } from "./FrameCollection";

export function Studio({
  w,
  images,
}: {
  w: Workspace;
  images: Record<string, string>;
}) {
  const [cropActive, setCropActive] = useState(false);
  const [cropRectangle, setCropRectangle] = useState<RegionRectV1>({
    x: 0,
    y: 0,
    width: 1,
    height: 1,
  });
  useEffect(() => {
    if (cropActive) {
      w.setZoomMode("fit");
      w.setZoom(1);
      w.setPan({ x: 0, y: 0 });
    }
  }, [cropActive]);
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
      <StudioToolbar w={w} cropEditing={cropActive && panel === "cloud"} />
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
                className={`image-viewport ${w.zoom > 1 || w.zoomMode === "pixels" ? "can-pan" : ""}`}
                onPointerDown={(e) => {
                  if (cropActive || (w.zoom <= 1 && w.zoomMode !== "pixels"))
                    return;
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
                  cropActive && panel === "cloud" ? (
                    <RegionEditor
                      src={api.image(w.image.imagePath)}
                      width={w.image.width}
                      height={w.image.height}
                      rectangle={cropRectangle}
                      onChange={setCropRectangle}
                    />
                  ) : (
                    <img
                      src={api.image(w.image.imagePath)}
                      alt={`Frame ${w.image.frameRef.presentationOrdinal} at ${timestamp(w.image.timeSeconds)}`}
                      style={{
                        ...(w.zoomMode === "pixels"
                          ? {
                              width:
                                w.image.width / (window.devicePixelRatio || 1),
                              height:
                                w.image.height / (window.devicePixelRatio || 1),
                              maxWidth: "none",
                              maxHeight: "none",
                            }
                          : {}),
                        transform: `translate(${w.pan.x}px,${w.pan.y}px) scale(${w.zoom})`,
                      }}
                      draggable={false}
                    />
                  )
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
              aria-pressed={panel === "cloud"}
              onClick={() => setPanel("cloud")}
            >
              Cloud AI
            </Button>
          </div>
          {panel === "cloud" ? (
            <CloudAnalysisPanel
              key={w.project?.id ?? "no-project"}
              w={w}
              region={[cropRectangle, setCropRectangle]}
              onCropChange={setCropActive}
            />
          ) : panel === "selections" ? (
            <FrameCollection w={w} images={images} />
          ) : (
            <AnalysisPanel
              w={w}
              onCompare={(frames, metrics) => {
                setComparison(frames);
                setComparisonMetrics(metrics);
              }}
            />
          )}
        </div>
      </div>
    </div>
  );
}
