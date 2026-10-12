import { useRef, useEffect, useState } from "react";
import { ChevronDown, ChevronUp, Film } from "lucide-react";
import { api, message, timestamp } from "../client";
import type { Workspace } from "../workspace/useWorkspace";

export function Timeline({
  w,
  expanded,
  onToggle,
}: {
  w: Workspace;
  expanded: boolean;
  onToggle: () => void;
}) {
  const wheel = useRef(0);
  const [nearby, setNearby] = useState<Workspace["thumbs"]>([]);
  const [nearbyError, setNearbyError] = useState("");
  useEffect(() => {
    let live = true;
    setNearby([]);
    setNearbyError("");
    if (!expanded || !w.sourceId || !w.frame || w.playing) return;
    const source = w.sourceId,
      n = BigInt(w.frame.ordinal),
      start = n > 4n ? n - 4n : 0n;
    void (async () => {
      const frames = await api.frames(source, start.toString(), 9);
      if (!live) return;
      setNearby(frames.map((frame) => ({ frame, url: "" })));
      for (const frame of frames) {
        if (!live) return;
        const image = await api.thumbnail(source, frame.ordinal);
        if (!live) return;
        setNearby((items) =>
          items.map((t) =>
            t.frame.ordinal === frame.ordinal
              ? { frame, url: api.image(image.imagePath) }
              : t,
          ),
        );
      }
    })().catch((e) => {
      if (live) setNearbyError(message(e));
    });
    return () => {
      live = false;
    };
  }, [
    expanded,
    w.sourceId,
    w.activeSource?.indexGeneration,
    w.frame?.ordinal,
    w.playing,
  ]);
  const thumbs = expanded ? nearby : w.thumbs;
  const shortTime = (seconds: number) =>
    `${Math.floor(seconds / 60)}:${(seconds % 60).toFixed(3).padStart(6, "0")}`;
  return (
    <section
      className={`timeline ${expanded ? "expanded" : ""}`}
      aria-label="Video timeline"
    >
      <div className="timeline-heading">
        <span>
          {w.activeSource
            ? `${w.activeSource.name} · ${w.activeSource.frameCount} indexed frames`
            : "No video attached"}
        </span>
        <span
          className="timeline-duration"
          title={`${timestamp(w.frame?.timeSeconds ?? 0)} / ${timestamp(w.activeSource?.metadata?.durationSeconds ?? 0)}`}
        >
          {shortTime(w.frame?.timeSeconds ?? 0)} /{" "}
          {w.activeSource?.metadata?.durationSeconds
            ? shortTime(w.activeSource.metadata.durationSeconds)
            : "—"}
        </span>
        <button
          className="text-button"
          onClick={onToggle}
          aria-expanded={expanded}
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
          {expanded ? "Collapse precision" : "Expand precision"}
        </button>
      </div>
      <div className="thumbnail-strip">
        {thumbs.length ? (
          thumbs.map((t) => (
            <button
              key={t.frame.ordinal}
              title={`Frame ${t.frame.ordinal} · ${timestamp(t.frame.timeSeconds)}`}
              aria-label={`Seek to frame ${t.frame.ordinal}`}
              aria-pressed={w.frame?.ordinal === t.frame.ordinal}
              onClick={() => {
                w.setPlaying(false);
                void w.show(w.sourceId!, t.frame.ordinal);
              }}
            >
              {t.url ? <img src={t.url} alt="" /> : <Film size={20} />}
              <span>
                {expanded
                  ? `#${t.frame.ordinal}`
                  : shortTime(t.frame.timeSeconds)}
              </span>
            </button>
          ))
        ) : (
          <div className="timeline-empty">
            {expanded
              ? w.playing
                ? "Pause to inspect neighboring frames."
                : nearbyError || "Loading neighboring frames…"
              : w.activeSource
                ? "Thumbnails appear as indexing finishes."
                : "Attach a video to display its timeline."}
          </div>
        )}
      </div>
      <input
        type="range"
        min={0}
        max={w.activeSource?.metadata?.durationSeconds ?? 1}
        step="0.001"
        value={w.frame?.timeSeconds ?? 0}
        aria-label="Timeline timestamp"
        disabled={!w.frame}
        onChange={(e) => {
          w.setPlaying(false);
          void w.seek(Number(e.target.value));
        }}
      />
      <div className="precision-row">
        <button
          className="precision-wheel"
          aria-label="Precision frame wheel: scroll to step frames"
          disabled={!w.frame}
          onWheel={(e) => {
            wheel.current += e.deltaY || e.deltaX;
            if (Math.abs(wheel.current) >= 12) {
              const steps = Math.max(
                -10,
                Math.min(10, Math.trunc(wheel.current / 12)),
              );
              wheel.current = 0;
              w.step(steps);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
              e.stopPropagation();
              e.preventDefault();
              w.step((e.key === "ArrowLeft" ? -1 : 1) * (e.shiftKey ? 10 : 1));
            }
          }}
        >
          <svg viewBox="0 0 560 32" aria-hidden="true">
            {Array.from({ length: 57 }, (_, i) => (
              <line
                key={i}
                x1={i * 10}
                x2={i * 10}
                y1={i % 5 === 0 ? 4 : 12}
                y2={28}
                stroke={
                  i === 28 ? "var(--color-focus)" : "rgba(255,255,255,.4)"
                }
                strokeWidth={i === 28 ? 3 : 1}
              />
            ))}
          </svg>
        </button>
        <span className="precision-hint">← → step · S select</span>
      </div>
      <details className="precision-guide">
        <summary>Navigation help</summary>
        <div>
          <strong>One decoded frame at a time</strong>
          <p>
            Scroll or use your trackpad to step. ← → moves one frame; Shift +
            arrows moves ten. Enter a timestamp above to resolve its exact
            presentation frame.
          </p>
        </div>
      </details>
    </section>
  );
}
