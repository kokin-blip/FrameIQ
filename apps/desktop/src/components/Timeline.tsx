import { useRef } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { timestamp } from "../client";
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
        <span className="timeline-duration">
          {w.activeSource?.metadata?.durationSeconds
            ? timestamp(w.activeSource.metadata.durationSeconds)
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
        {w.thumbs.length ? (
          w.thumbs.map((t) => (
            <button
              key={t.frame.ordinal}
              aria-label={`Seek to frame ${t.frame.ordinal}`}
              aria-pressed={w.frame?.ordinal === t.frame.ordinal}
              onClick={() => {
                w.setPlaying(false);
                void w.show(w.sourceId!, t.frame.ordinal);
              }}
            >
              <img src={t.url} alt="" />
              <span>{timestamp(t.frame.timeSeconds)}</span>
            </button>
          ))
        ) : (
          <div className="timeline-empty">
            {w.activeSource
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
      {expanded && (
        <div className="precision-guide">
          <strong>One decoded frame at a time</strong>
          <p>
            Scroll or use your trackpad to step. ← → moves one frame; Shift +
            arrows moves ten. Enter a timestamp above to resolve its exact
            presentation frame.
          </p>
        </div>
      )}
    </section>
  );
}
