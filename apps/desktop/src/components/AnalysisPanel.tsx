import { useEffect, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, message, timestamp } from "../client";
import { useJobs } from "../store";
import type {
  QualityMetricsV1,
  RankingConfigV1,
  RecommendationSetV1,
  Selection,
} from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";
const defaults: RankingConfigV1 = {
  count: 8,
  sharpnessWeight: 50,
  exposureWeight: 30,
  blurWeight: 20,
};
export function AnalysisPanel({
  w,
  onCompare,
}: {
  w: Workspace;
  onCompare: (
    frames: Selection[],
    metrics: Record<string, QualityMetricsV1>,
  ) => void;
}) {
  const [config, setConfig] = useState(defaults);
  const [result, setResult] = useState<RecommendationSetV1 | null>(null);
  const [chosen, setChosen] = useState<Selection[]>([]);
  const [loading, setLoading] = useState(false);
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob);
  const active = Object.values(jobs).find(
    (j) =>
      j.stage === "analyzing" &&
      j.resultIds.includes(w.sourceId ?? "") &&
      ["queued", "running", "cancelling"].includes(j.status),
  );
  const analysisState=Object.values(jobs).filter(j=>j.stage==="analyzing"&&j.resultIds.includes(w.sourceId??"")).map(j=>`${j.jobId}:${j.status}`).join("|");
  const [poll,setPoll]=useState(0);
  useEffect(()=>{if(!active)return;const timer=setInterval(()=>setPoll(n=>n+1),2500);return()=>clearInterval(timer);},[active?.jobId]);
  useEffect(() => {
    setChosen([]);
    setResult(null);
  }, [w.sourceId]);
  useEffect(() => {
    if (!w.sourceId) return;
    let alive = true;
    const timeout = setTimeout(() => {
      setLoading(true);
      api
        .recommendations(w.sourceId!, config)
        .then((r) => {
          if (alive) setResult(r);
        })
        .catch((e) => {
          if (alive) w.setError(message(e));
        })
        .finally(() => {
          if (alive) setLoading(false);
        });
    }, 200);
    return () => {
      alive = false;
      clearTimeout(timeout);
    };
  }, [w.sourceId, config, analysisState, poll]);
  async function analyze() {
    if (!w.sourceId) return;
    try {
      setJob(await api.analyze(w.sourceId));
    } catch (e) {
      w.setError(message(e));
    }
  }
  async function select(frame: Selection) {
    try {
      await api.select(
        frame.sourceId,
        frame.frameRef.presentationOrdinal,
        true,
      );
    } catch (e) {
      w.setError(message(e));
    }
  }
  return (
    <aside className="analysis-panel" aria-label="Frame recommendations">
      <h2>Recommendations</h2>
      <Button
        variant="primary"
        disabled={
          !w.activeSource || w.activeSource.status !== "ready" || !!active
        }
        onClick={() => void analyze()}
      >
        Analyze source
      </Button>
      {active && (
        <div role="status">
          <p>
            {active.messageCode.toLowerCase().replaceAll("_", " ")} ·{" "}
            {active.completedUnits}/{active.totalUnits ?? "?"}
          </p>
          <Button
            disabled={active.status === "cancelling"}
            onClick={() =>
              void api.cancel(active.jobId).catch((e) => w.setError(message(e)))
            }
          >
            Cancel
          </Button>
        </div>
      )}
      <fieldset>
        <legend>Ranking</legend>
        {(
          [
            ["count", "Frame count"],
            ["sharpnessWeight", "Sharpness"],
            ["exposureWeight", "Exposure"],
            ["blurWeight", "Blur estimate"],
          ] as const
        ).map(([key, label]) => (
          <label key={key}>
            {label}
            <input
              type="number"
              min={key === "count" ? 1 : 0}
              max={key === "count" ? 50 : 100}
              value={config[key]}
              onChange={(e) =>
                setConfig((c) => ({ ...c, [key]: Number(e.target.value) }))
              }
            />
          </label>
        ))}
      </fieldset>
      <p className="analysis-note">
        Weights reuse saved measurements. Scores are quality estimates.
      </p>
      {result && !result.coverageComplete && (
        <p role="status">Partial results · analysis {result.status}</p>
      )}
      {!result && !active && (
        <p>Analyze this source to find distinct frames.</p>
      )}
      {loading && <p role="status">Ranking frames…</p>}
      <Button
        disabled={chosen.length < 2}
        onClick={() =>
          onCompare(
            chosen,
            Object.fromEntries(
              result?.frames.map((r) => [
                `${r.quality.frame.sourceId}:${r.quality.frame.frameRef.presentationOrdinal}`,
                r.quality.metrics,
              ]) ?? [],
            ),
          )
        }
      >
        Compare {chosen.length || "frames"}
      </Button>
      <div className="recommendation-list">
        {result?.frames.map((r) => {
          const frame = r.quality.frame,
            key = frame.frameRef.presentationOrdinal;
          const checked = chosen.some(
            (s) => s.frameRef.presentationOrdinal === key,
          );
          return (
            <article className="recommendation-row" key={key}>
              <button onClick={() => w.openSelection(frame)}>
                Frame {key} · {timestamp(frame.timeSeconds)}
              </button>
              <strong>Quality {r.score.toFixed(0)}</strong>
              <p>{r.reasons.join(" · ")}</p>
              <div>
                <Button onClick={() => void select(frame)}>
                  Add to selection
                </Button>
                <label>
                  <input
                    type="checkbox"
                    checked={checked}
                    disabled={!checked && chosen.length >= 4}
                    onChange={() =>
                      setChosen((s) =>
                        checked
                          ? s.filter(
                              (f) => f.frameRef.presentationOrdinal !== key,
                            )
                          : [...s, frame],
                      )
                    }
                  />
                  Compare
                </label>
              </div>
            </article>
          );
        })}
      </div>
      {result && !result.frames.length && <p>No usable measurements yet.</p>}
    </aside>
  );
}
