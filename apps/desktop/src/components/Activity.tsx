import { useState } from "react";
import { Check, ChevronUp, X } from "lucide-react";
import { api, desktop, message } from "../client";
import type { Workspace } from "../workspace/useWorkspace";

export function Activity({ w }: { w: Workspace }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <footer className="activity-strip">
      <button
        className="text-button"
        aria-expanded={expanded}
        aria-controls="activity-panel"
        onClick={() => setExpanded((v) => !v)}
      >
        {w.activeJobs.length ? (
          <span className="activity-dot" />
        ) : (
          <Check size={14} />
        )}
        {w.activeJobs.length
          ? `${w.activeJobs.length} active ${w.activeJobs.length === 1 ? "job" : "jobs"}`
          : desktop
            ? "Ready"
            : "Design preview · launch desktop for file access"}
        <ChevronUp size={14} />
        <span>Activity</span>
      </button>
      {expanded && (
        <section
          id="activity-panel"
          className="activity-panel"
          aria-label="Background activity"
        >
          <div className="panel-heading">
            <h2>Background activity</h2>
            <button
              className="icon-button"
              aria-label="Close activity"
              onClick={() => setExpanded(false)}
            >
              <X size={16} />
            </button>
          </div>
          <div aria-live="polite">
            {w.activeJobs.length ? (
              w.activeJobs.map((j) => (
                <div className="job-row" key={j.jobId}>
                  <div>
                    <strong>{j.stage}</strong>
                    <p>
                      {j.completedUnits}
                      {j.totalUnits ? ` / ${j.totalUnits}` : " frames"} ·{" "}
                      {j.status}
                    </p>
                  </div>
                  <button
                    className="text-button"
                    disabled={j.status === "cancelling"}
                    onClick={() =>
                      api.cancel(j.jobId).catch((e) => w.setError(message(e)))
                    }
                  >
                    {j.status === "cancelling" ? "Cancelling…" : "Cancel"}
                  </button>
                </div>
              ))
            ) : (
              <p>No active jobs.</p>
            )}
          </div>
        </section>
      )}
      <span className="sr-only" aria-live="polite">
        {w.activeJobs.map((j) => `${j.stage}: ${j.status}`).join(". ")}
      </span>
    </footer>
  );
}
