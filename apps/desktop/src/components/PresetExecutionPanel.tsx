import { fashionApi } from "../fashionClient";
import { useModelRegistry } from "../workspace/modelRegistry";
import { useEffect, useRef, useState } from "react";
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
  const models = useModelRegistry((s) => s.models);
  const [choice, setChoice] = useState<SubjectChoiceV1 | null>(null);
  const [face, setFace] = useState("");
  const [task, setTask] = useState<string>();
  const [result, setResult] = useState<PresetResultV1 | null>(null);
  const [starting, setStarting] = useState(false);
  const [selectionStatus, setSelectionStatus] = useState("");
  const applied = useRef(new Set<string>());
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob);
  const ordinal = w.image?.frameRef.presentationOrdinal;
  const source = w.sourceId;
  const active = task ? jobs[task] : undefined;
  const running =
    starting ||
    (!!active && ["queued", "running", "cancelling"].includes(active.status));
  const kind = preset.requiredCapabilities.join(",");
  const [extraCapabilities, setExtraCapabilities] = useState<string[]>([]);
  const baselineSupported = [
    "region_tracking",
    "face_landmarks",
    "person,pose,person_mask",
  ].includes(kind);
  const hasPose = models.some(
    (m) => m.status === "ready" && m.manifest.task === "pose",
  );
  const supported =
    baselineSupported ||
    (kind === "head_region,hair_mask" && extraCapabilities.includes("hair")) ||
    (["accessory_region", "foot_landmarks,footwear_region"].includes(kind) &&
      extraCapabilities.includes("detection")) ||
    (kind === "person,pose,view_orientation" && hasPose);
  useEffect(() => {
    let live = true;
    if (!baselineSupported)
      void fashionApi
        .capabilities()
        .then((c) => {
          if (live)
            setExtraCapabilities(
              c.filter((c) => c.status !== "unavailable").map((c) => c.task),
            );
        })
        .catch(() => {
          if (live) setExtraCapabilities([]);
        });
    return () => {
      live = false;
    };
  }, [kind, jobs]);
  useEffect(() => {
    let live = true;
    setChoice(null);
    if (source && ordinal)
      void Promise.all([
        useModelRegistry.getState().refresh(),
        api.subjectChoice(source, ordinal),
      ])
        .then(([list, selected]) => {
          if (!live) return;
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
    setSelectionStatus("");
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
        .then(async (r) => {
          if (!live) return;
          setResult(r);
          if (
            r?.frames.length &&
            active.status === "succeeded" &&
            !applied.current.has(task)
          ) {
            applied.current.add(task);
            try {
              await w.refreshSelections();
              if (live) setSelectionStatus("Preset results selected.");
            } catch (e) {
              if (live)
                setSelectionStatus(
                  "Could not refresh Selections. Reopen the project or retry below.",
                );
            }
          } else if (r && active.status === "cancelled") {
            setSelectionStatus(
              "Execution cancelled. Results were not added automatically.",
            );
          }
        })
        .catch((e) => {
          if (live) w.setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [task, active?.status]);
  const faces = models.filter((m) =>
    ["yunet", "face", "face_detector"].includes(m.manifest.task),
  );
  const faceId = face || faces[0]?.id;
  async function run() {
    if (!source || !ordinal || !choice || !experimental) return;
    setStarting(true);
    try {
      const job = await api.selectPresetFrames({
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
      setSelectionStatus("");
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
      <h3>Automatic frame selection</h3>
      <p>
        Choose the anchor subject above. Tracking runs in both directions and
        stops at cuts, lost subjects, or ambiguity. Accuracy is not yet
        validated. Results are added to Selections; existing frames are kept.
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
          Face detection model
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
      {kind === "person,pose,person_mask" && !hasPose && (
        <p>
          Degraded Outfit mode: visibility and coverage use tracked-region
          geometry proxies until a pose model is available.
        </p>
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
          w.selectionBusy ||
          !supported ||
          !experimental ||
          !choice ||
          (kind === "region_tracking" && !rectangle) ||
          (kind === "face_landmarks" && !faceId)
        }
        onClick={() => void run()}
      >
        Auto-select frames
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
          {selectionStatus && <p role="status">{selectionStatus}</p>}
          {result.frames.length > 0 && (
            <Button
              disabled={w.selectionBusy || running}
              onClick={() =>
                void w
                  .addSelections(result.frames.map((f) => f.quality.frame))
                  .then(() => setSelectionStatus("Preset results selected."))
                  .catch(() =>
                    setSelectionStatus(
                      "Could not add preset results. Retry below.",
                    ),
                  )
              }
            >
              Add preset results
            </Button>
          )}
          <p role="status">
            {result.track.coverageComplete
              ? "Sampled coverage complete"
              : "Partial coverage"}
            {result.track.stopReason
              ? ` · ${result.track.stopReason.replaceAll("_", " ")}`
              : ""}
          </p>
          <p>
            All five weights affect ranking. Visibility, coverage, occlusion and
            view orientation are experimental measurements; geometry proxies are
            labeled in each result.
          </p>
          {result.preset.requiredCapabilities.includes("view_orientation") && (
            <div>
              <h4>Complementary detail crops</h4>
              {!result.details?.length && (
                <p>
                  No eligible distinct detail crops were available. Install a
                  qualified detection pack or collect closer views.
                </p>
              )}
              {result.details?.map((item) => (
                <article key={item.id}>
                  {item.crop && (
                    <img
                      style={{
                        maxWidth: "100%",
                        maxHeight: 160,
                        objectFit: "contain",
                      }}
                      src={api.image(item.crop.path)}
                      alt={`${item.category} detail crop`}
                    />
                  )}
                  <Button
                    onClick={() => {
                      if (item.media.kind === "frame")
                        w.openSelection(item.media.selection);
                    }}
                  >
                    {item.category.replaceAll("_", " ")} · inspect exact source
                  </Button>
                </article>
              ))}
            </div>
          )}
          {result.frames.map((r) => (
            <article
              className="recommendation-row"
              key={r.quality.frame.frameRef.presentationOrdinal}
            >
              <button onClick={() => w.openSelection(r.quality.frame)}>
                Frame {r.quality.frame.frameRef.presentationOrdinal} ·{" "}
                {timestamp(r.quality.frame.timeSeconds)}
              </button>
              <strong>Weighted score {r.score.toFixed(0)}</strong>
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
