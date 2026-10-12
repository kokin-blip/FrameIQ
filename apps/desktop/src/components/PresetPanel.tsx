import { PresetExecutionPanel } from "./PresetExecutionPanel";
import { SubjectPanel } from "./SubjectPanel";
import { useEffect, useRef, useState } from "react";
import { api, message, timestamp } from "../client";
import { useJobs } from "../store";
import type { GuidedRegionV1, PresetV1, RegionRectV1 } from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";
export function PresetPanel({ w }: { w: Workspace }) {
  const [presets, setPresets] = useState<PresetV1[]>([]),
    [id, setId] = useState("custom"),
    [regions, setRegions] = useState<GuidedRegionV1[]>([]),
    [rectangle, setRectangle] = useState<RegionRectV1>(),
    [name, setName] = useState("");
  const pendingRegion = useRef<GuidedRegionV1 | null>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  const setJob = useJobs((s) => s.setJob);
  const jobs = useJobs((s) => s.jobs);
  const selected = presets.find((p) => p.id === id);
  useEffect(() => {
    void api
      .presets()
      .then(setPresets)
      .catch((e) => w.setError(message(e)));
  }, []);
  useEffect(() => {
    const pending = pendingRegion.current;
    if (pending) {
      if (
        pending.frame.sourceId === w.sourceId &&
        pending.frame.frameRef.presentationOrdinal ===
          w.image?.frameRef.presentationOrdinal
      ) {
        setRectangle(pending.rectangle);
        pendingRegion.current = null;
      }
    } else setRectangle(undefined);
  }, [w.image?.frameRef.presentationOrdinal, w.sourceId]);
  useEffect(() => {
    if (w.sourceId)
      void api
        .regions(w.sourceId)
        .then(setRegions)
        .catch((e) => w.setError(message(e)));
  }, [w.sourceId, jobs]);
  function point(e: React.PointerEvent<SVGSVGElement>) {
    const svg = e.currentTarget;
    const p = svg.createSVGPoint();
    p.x = e.clientX;
    p.y = e.clientY;
    const matrix = svg.getScreenCTM();
    if (!matrix || !w.image) return null;
    const v = p.matrixTransform(matrix.inverse());
    return {
      x: Math.max(0, Math.min(1, v.x / w.image.width)),
      y: Math.max(0, Math.min(1, v.y / w.image.height)),
    };
  }
  async function saveRegion() {
    if (!rectangle || !w.sourceId || !w.image) return;
    try {
      setJob(
        await api.measureRegion(
          w.sourceId,
          w.image.frameRef.presentationOrdinal,
          id,
          rectangle,
        ),
      );
    } catch (e) {
      w.setError(message(e));
    }
  }
  return (
    <aside className="analysis-panel" aria-label="Intelligent presets">
      <h2>Preset settings</h2>
      <label>
        Preset
        <select value={id} onChange={(e) => setId(e.target.value)}>
          {presets.map((p) => (
            <option value={p.id} key={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
      <SubjectPanel w={w} />
      {selected && (
        <>
          <p role="status">{selected.unavailableReason}</p>
          <details>
            <summary>Weights and custom presets</summary>
            <p>Required: {selected.requiredCapabilities.join(", ")}</p>
            <fieldset>
              <legend>Preset weights</legend>
              {[
                "Sharpness",
                "Lighting",
                "Target visibility",
                "Coverage",
                "Composition",
              ].map((label, i) => (
                <label key={label}>
                  {label}
                  <input
                    type="number"
                    min="0"
                    max="100"
                    value={selected.weights[i]}
                    onChange={(e) =>
                      setPresets((s) =>
                        s.map((p) =>
                          p.id === id
                            ? {
                                ...p,
                                weights: p.weights.map((n, j) =>
                                  j === i ? Number(e.target.value) : n,
                                ) as PresetV1["weights"],
                              }
                            : p,
                        ),
                      )
                    }
                  />
                </label>
              ))}
              <label>
                Frame count
                <input
                  type="number"
                  min="1"
                  max="50"
                  value={selected.count}
                  onChange={(e) =>
                    setPresets((s) =>
                      s.map((p) =>
                        p.id === id
                          ? { ...p, count: Number(e.target.value) }
                          : p,
                      ),
                    )
                  }
                />
              </label>
            </fieldset>
            <label>
              Custom preset name
              <input value={name} onChange={(e) => setName(e.target.value)} />
            </label>
            <button
              disabled={!name.trim()}
              onClick={() =>
                void api
                  .savePreset({ ...selected, name: name.trim() })
                  .then((p) => {
                    setPresets((s) => [...s, p]);
                    setId(p.id);
                    setName("");
                  })
                  .catch((e) => w.setError(message(e)))
              }
            >
              Save custom preset
            </button>
          </details>
        </>
      )}
      {selected && (
        <PresetExecutionPanel w={w} preset={selected} rectangle={rectangle} />
      )}
      <h3>Guided region</h3>
      <p>
        Draw or enter a rectangle for the selected subject. It applies only to
        this frame. You can also measure a region without choosing a subject.
      </p>
      {w.image && (
        <svg
          className="region-editor"
          viewBox={`0 0 ${w.image.width} ${w.image.height}`}
          role="img"
          aria-label="Draw a region on the current frame"
          onPointerDown={(e) => {
            start.current = point(e);
            e.currentTarget.setPointerCapture(e.pointerId);
          }}
          onPointerMove={(e) => {
            const p = point(e);
            if (p && start.current) {
              setRectangle({
                x: Math.min(p.x, start.current.x),
                y: Math.min(p.y, start.current.y),
                width: Math.abs(p.x - start.current.x),
                height: Math.abs(p.y - start.current.y),
              });
            }
          }}
          onPointerUp={() => {
            start.current = null;
          }}
          onPointerCancel={() => {
            start.current = null;
          }}
        >
          <image
            href={api.image(w.image.imagePath)}
            width={w.image.width}
            height={w.image.height}
          />
          {rectangle && (
            <rect
              x={rectangle.x * w.image.width}
              y={rectangle.y * w.image.height}
              width={rectangle.width * w.image.width}
              height={rectangle.height * w.image.height}
              fill="rgba(40,140,255,.15)"
              stroke="#3395ff"
              strokeWidth={Math.max(2, w.image.width / 200)}
            />
          )}
        </svg>
      )}
      <fieldset>
        <legend>Rectangle (fraction of image)</legend>
        {(["x", "y", "width", "height"] as const).map((key) => (
          <label key={key}>
            {key}
            <input
              type="number"
              min="0"
              max="1"
              step=".01"
              value={rectangle?.[key] ?? 0}
              onChange={(e) =>
                setRectangle((r) => ({
                  ...(r ?? { x: 0, y: 0, width: 0, height: 0 }),
                  [key]: Number(e.target.value),
                }))
              }
            />
          </label>
        ))}
      </fieldset>
      <button
        disabled={!w.image || !rectangle?.width || !rectangle?.height}
        onClick={() => void saveRegion()}
      >
        Measure region
      </button>
      {regions
        .filter((r) => r.presetId === id)
        .map((r) => (
          <article className="recommendation-row" key={r.id}>
            <button
              onClick={() => {
                pendingRegion.current = r;
                w.openSelection(r.frame);
                if (
                  w.sourceId === r.frame.sourceId &&
                  w.image?.frameRef.presentationOrdinal ===
                    r.frame.frameRef.presentationOrdinal
                ) {
                  setRectangle(r.rectangle);
                  pendingRegion.current = null;
                }
              }}
            >
              Frame {r.frame.frameRef.presentationOrdinal} ·{" "}
              {timestamp(r.frame.timeSeconds)}
            </button>
            {r.subject && (
              <p>Person {r.subject.personIndex + 1} · this frame</p>
            )}
            <p>
              Region sharpness {r.metrics.sharpness.toFixed(0)} · unclipped{" "}
              {(r.metrics.exposure * 100).toFixed(0)}%
            </p>
            <button
              onClick={() =>
                void api
                  .removeRegion(r.id)
                  .then(() => setRegions((s) => s.filter((x) => x.id !== r.id)))
                  .catch((e) => w.setError(message(e)))
              }
            >
              Remove region
            </button>
          </article>
        ))}
    </aside>
  );
}
