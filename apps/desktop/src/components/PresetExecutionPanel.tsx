import { useEffect, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, message, timestamp } from "../client";
import { useJobs } from "../store";
import type {
  ModelInstallationV1,
  PresetV1,
  PresetResultV1,
  RegionRectV1,
  SubjectChoiceV1,
} from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";

export function PresetExecutionPanel({
  w,
  preset,
  rectangle,
}: {
  w: Workspace;
  preset: PresetV1;
  rectangle?: RegionRectV1;
}) {
  const [experimental, setExperimental] = useState(false);
  const [models, setModels] = useState<ModelInstallationV1[]>([]);
  const [choice, setChoice] = useState<SubjectChoiceV1 | null>(null);
  const [face, setFace] = useState("");
  const [task, setTask] = useState<string>();
  const [result, setResult] = useState<PresetResultV1 | null>(null);
  const [starting, setStarting] = useState(false);
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob);
  const ordinal = w.image?.frameRef.presentationOrdinal;
  const source = w.sourceId;
  const active = task ? jobs[task] : undefined;
  const running =
    starting ||
    (!!active && ["queued", "running", "cancelling"].includes(active.status));
  const kind = preset.requiredCapabilities.join(",");
  const supported = [
    "region_tracking",
    "face_landmarks",
    "person,pose,person_mask",
  ].includes(kind);
  useEffect(() => {
    let live = true;
    setChoice(null);
    if (source && ordinal)
      void Promise.all([api.models(), api.subjectChoice(source, ordinal)])
        .then(([list, selected]) => {
          if (!live) return;
          setModels(list.filter((m) => m.status === "ready"));
          setChoice(selected);
        })
        .catch((e) => {
          if (live) w.setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [source, ordinal, jobs]);
  useEffect(() => {
    setTask(undefined);
    setResult(null);
    setExperimental(false);
  }, [source, preset.id]);
  useEffect(() => {
    let live = true;
    if (
      task &&
      active &&
      ["succeeded", "failed", "cancelled"].includes(active.status)
    )
      void api
        .presetResult(task)
        .then((r) => {
          if (live) setResult(r);
        })
        .catch((e) => {
          if (live) w.setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [task, active?.status]);
  const faces = models.filter((m) => m.manifest.task === "face_detector");
  const faceId = face || faces[0]?.id;
  async function run() {
    if (!source || !ordinal || !choice || !experimental) return;
    setStarting(true);
    try {
      const job = await api.executePreset({
        sourceId: source,
        preset,
        anchorOrdinal: ordinal,
        personModelId: choice.modelId,
        personIndex: choice.subject.personIndex,
        faceModelId: kind === "face_landmarks" ? (faceId ?? null) : null,
        customRectangle:
          kind === "region_tracking" ? (rectangle ?? null) : null,
        experimental,
      });
      setResult(null);
      setTask(job.jobId);
      setJob(job);
    } catch (e) {
      w.setError(message(e));
    } finally {
      setStarting(false);
    }
  }
  return (
    <section aria-label="Experimental preset execution">
      <h3>Experimental preset execution</h3>
      <p>
        Choose the anchor subject above. Tracking stops at cuts, lost subjects,
        or ambiguity. Accuracy is not yet validated.
      </p>
      <label>
        <input
          type="checkbox"
          checked={experimental}
          disabled={running}
          onChange={(e) => setExperimental(e.target.checked)}
        />{" "}
        Allow experimental tracking
      </label>
      {kind === "face_landmarks" && (
        <label>
          Face detector
          <select
            value={faceId ?? ""}
            onChange={(e) => setFace(e.target.value)}
          >
            <option value="">No face model</option>
            {faces.map((m) => (
              <option key={m.id} value={m.id}>
                {m.manifest.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {!supported && (
        <p>Required automatic model capability unavailable for this preset.</p>
      )}
      {!choice && (
        <p>Detect people and choose a subject on the current frame.</p>
      )}
      <Button
        variant="primary"
        disabled={
          running ||
          !supported ||
          !experimental ||
          !choice ||
          (kind === "region_tracking" && !rectangle) ||
          (kind === "face_landmarks" && !faceId)
        }
        onClick={() => void run()}
      >
        Run from current frame
      </Button>
      {running && active && (
        <>
          <p role="status">
            {active.completedUnits} / {active.totalUnits ?? "…"} ·{" "}
            {active.status}
          </p>
          <Button
            disabled={active.status === "cancelling"}
            onClick={() =>
              void api.cancel(active.jobId).catch((e) => w.setError(message(e)))
            }
          >
            Cancel execution
          </Button>
        </>
      )}
      {active?.error && <p role="alert">{active.error.message}</p>}
      {result && (
        <>
          <p role="status">
            {result.track.coverageComplete
              ? "Sampled coverage complete"
              : "Partial coverage"}
            {result.track.stopReason
              ? ` · ${result.track.stopReason.replaceAll("_", " ")}`
              : ""}
          </p>
          <p>
            Region quality only. Visibility, coverage, and composition weights
            are not scored.
          </p>
          {result.frames.map((r) => (
            <article
              className="recommendation-row"
              key={r.quality.frame.frameRef.presentationOrdinal}
            >
              <button onClick={() => w.openSelection(r.quality.frame)}>
                Frame {r.quality.frame.frameRef.presentationOrdinal} ·{" "}
                {timestamp(r.quality.frame.timeSeconds)}
              </button>
              <strong>Region quality {r.score.toFixed(0)}</strong>
              <p>{r.reasons.join(" · ")}</p>
            </article>
          ))}
          {!result.frames.length && (
            <p>No eligible target-region measurements.</p>
          )}
        </>
      )}
    </section>
  );
}
