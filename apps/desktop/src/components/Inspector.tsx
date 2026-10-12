import { timestamp } from "../client";
import type { Workspace } from "../workspace/useWorkspace";

export function Inspector({ w }: { w: Workspace }) {
  return (
    <section
      id="frame-inspector"
      className="frame-inspector"
      aria-label="Frame details"
    >
      <div className="panel-heading">
        <h2>Frame details</h2>
      </div>
      <dl>
        <dt>Presentation ordinal</dt>
        <dd>{w.frame?.ordinal ?? "—"}</dd>
        <dt>Resolution</dt>
        <dd>
          {w.image
            ? `${w.image.width} × ${w.image.height}`
            : w.currentStream
              ? `${w.currentStream.width} × ${w.currentStream.height}`
              : "—"}
        </dd>
        <dt>Presentation time</dt>
        <dd>{w.frame ? timestamp(w.frame.timeSeconds) : "—"}</dd>
      </dl>
      <details>
        <summary>Frame metadata</summary>
        <dl>
          <dt>PTS ticks</dt>
          <dd>{w.frame?.ptsTicks ?? "Unknown"}</dd>
          <dt>Timestamp origin</dt>
          <dd>{w.frame?.timestampOrigin ?? "—"}</dd>
          <dt>Index</dt>
          <dd>{w.activeSource?.status ?? "No source"}</dd>
        </dl>
      </details>
    </section>
  );
}
