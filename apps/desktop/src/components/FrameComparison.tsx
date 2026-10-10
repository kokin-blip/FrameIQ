import { useEffect, useRef, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, message, timestamp } from "../client";
import type { FrameView, Selection, QualityMetricsV1 } from "../contracts";
export function FrameComparison({
  frames,
  onClose,
  onError,
  metrics,
}: {
  frames: Selection[];
  metrics?: Record<string, QualityMetricsV1>;
  onClose: () => void;
  onError: (message: string) => void;
}) {
  const [images, setImages] = useState<FrameView[]>([]),
    [zoom, setZoom] = useState(1),
    [actual, setActual] = useState(false),
    [pan, setPan] = useState({ x: 0, y: 0 });
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(
    null,
  );
  useEffect(() => {
    let alive = true;
    setImages([]);
    void (async () => {
      const loaded = [];
      for (const f of frames) {
        loaded.push(
          await api.preview(f.sourceId, f.frameRef.presentationOrdinal),
        );
        if (!alive) return;
        setImages([...loaded]);
      }
    })().catch((e) => {
      if (alive) onError(message(e));
    });
    return () => {
      alive = false;
    };
  }, [frames, onError]);
  return (
    <section className="comparison" aria-label="Frame comparison">
      <div className="comparison-toolbar">
        <Button onClick={onClose}>Close comparison</Button>
        <label>
          Linked zoom{" "}
          <input
            type="range"
            min={1}
            max={4}
            step={0.1}
            value={zoom}
            onChange={(e) => setZoom(Number(e.target.value))}
          />
        </label>
        <Button
          onClick={() => {
            setActual(false);
            setZoom(1);
            setPan({ x: 0, y: 0 });
          }}
        >
          Fit / reset
        </Button>
        <Button
          onClick={() => {
            setActual(true);
            setZoom(1);
            setPan({ x: 0, y: 0 });
          }}
        >
          Actual pixels
        </Button>
        <span>
          {Math.round(zoom * 100)}% {actual ? "of original pixels" : "of fit"}
        </span>
      </div>
      <div className="comparison-grid">
        {frames.map((f, i) => (
          <article key={`${f.sourceId}-${f.frameRef.presentationOrdinal}`}>
            <div
              className="comparison-image"
              tabIndex={0}
              aria-label={`Pan compared frame ${f.frameRef.presentationOrdinal}`}
              onKeyDown={(e) => {
                const directions: Record<string, number[]> = {
                  ArrowLeft: [-0.05, 0],
                  ArrowRight: [0.05, 0],
                  ArrowUp: [0, -0.05],
                  ArrowDown: [0, 0.05],
                };
                const d = directions[e.key];
                if (d) {
                  e.preventDefault();
                  setPan((p) => ({ x: p.x + d[0], y: p.y + d[1] }));
                }
              }}
              onPointerDown={(e) => {
                if (zoom <= 1 && !actual) return;
                drag.current = {
                  x: e.clientX,
                  y: e.clientY,
                  px: pan.x,
                  py: pan.y,
                };
                e.currentTarget.setPointerCapture(e.pointerId);
              }}
              onPointerMove={(e) => {
                if (drag.current)
                  setPan({
                    x:
                      drag.current.px +
                      (e.clientX - drag.current.x) /
                        e.currentTarget.clientWidth,
                    y:
                      drag.current.py +
                      (e.clientY - drag.current.y) /
                        e.currentTarget.clientHeight,
                  });
              }}
              onPointerUp={() => {
                drag.current = null;
              }}
              onPointerCancel={() => {
                drag.current = null;
              }}
            >
              {images[i] ? (
                <img
                  src={api.image(images[i].imagePath)}
                  alt={`Compared frame ${f.frameRef.presentationOrdinal}`}
                  draggable={false}
                  style={{
                    maxWidth: actual ? "none" : "100%",
                    maxHeight: actual ? "none" : "100%",
                    width: actual ? images[i].width : undefined,
                    height: actual ? images[i].height : undefined,
                    flexShrink: 0,
                    transform: `translate(${pan.x * 100}%,${pan.y * 100}%) scale(${zoom})`,
                  }}
                />
              ) : (
                <span>Loading frame…</span>
              )}
            </div>
            <p>
              Frame {f.frameRef.presentationOrdinal} ·{" "}
              {timestamp(f.timeSeconds)}
            </p>
            <small>
              {images[i] ? `${images[i].width} × ${images[i].height}` : ""} ·
              PTS {f.frameRef.ptsTicks ?? "missing"} ·{" "}
              {f.frameRef.timestampOrigin}
            </small>
            {metrics?.[`${f.sourceId}:${f.frameRef.presentationOrdinal}`] && (
              <small>
                Sharpness{" "}
                {metrics[
                  `${f.sourceId}:${f.frameRef.presentationOrdinal}`
                ].sharpness.toFixed(0)}{" "}
                · unclipped{" "}
                {(
                  metrics[`${f.sourceId}:${f.frameRef.presentationOrdinal}`]
                    .exposure * 100
                ).toFixed(0)}
                %
              </small>
            )}
          </article>
        ))}
      </div>
    </section>
  );
}
