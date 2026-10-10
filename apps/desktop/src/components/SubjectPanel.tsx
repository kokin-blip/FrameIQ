import { useEffect, useRef, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, message } from "../client";
import { useJobs } from "../store";
import type {
  ModelInstallationV1,
  PersonFrameV1,
  PoseFrameV1,
  SubjectChoiceV1,
} from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";

export function SubjectPanel({ w }: { w: Workspace }) {
  const [models, setModels] = useState<ModelInstallationV1[]>([]);
  const [model, setModel] = useState("");
  const [poseModel, setPoseModel] = useState("");
  const [peopleCache, setPeople] = useState<PersonFrameV1 | null>(null);
  const [choice, setChoice] = useState<SubjectChoiceV1 | null>(null);
  const [poseCache, setPose] = useState<PoseFrameV1 | null>(null);
  const [error, setError] = useState("");
  const [reading, setReading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [showMask, setShowMask] = useState(false);
  const [task, setTask] = useState<{ id: string; frame: string } | null>(null);
  const jobs = useJobs((s) => s.jobs);
  const setJob = useJobs((s) => s.setJob);
  const terminal = Object.values(jobs)
    .filter(
      (j) =>
        ["succeeded", "failed", "cancelled"].includes(j.status) &&
        [
          "person_detection",
          "pose_estimation",
          "model_install",
          "model_download",
        ].includes(j.stage),
    )
    .map((j) => `${j.jobId}:${j.sequence}`)
    .join("|");
  const image = w.image;
  const source = w.sourceId;
  const ordinal = image?.frameRef.presentationOrdinal;
  const frameKey =
    source &&
    image &&
    image.frameRef.indexGeneration === w.activeSource?.indexGeneration
      ? `${source}:${image.frameRef.indexGeneration}:${ordinal}`
      : "";
  const currentKey = useRef(frameKey);
  currentKey.current = frameKey;
  const modelIntent = useRef(0);
  const currentModel = useRef(model);
  currentModel.current = model;
  const request = useRef(0);
  const job = task ? jobs[task.id] : undefined;
  const running =
    !!job && ["queued", "running", "cancelling"].includes(job.status);
  const people =
    peopleCache &&
    peopleCache.modelId === model &&
    peopleCache.frame.sourceId === source &&
    peopleCache.frame.frameRef.indexGeneration ===
      image?.frameRef.indexGeneration &&
    peopleCache.frame.frameRef.presentationOrdinal === ordinal
      ? peopleCache
      : null;
  const selected =
    people && choice?.subject.personFeatureId === people.id
      ? choice.subject.personIndex
      : null;
  const pose =
    poseCache &&
    poseCache.personFeatureId === people?.id &&
    poseCache.personIndex === selected &&
    poseCache.modelId === poseModel
      ? poseCache
      : null;
  useEffect(() => {
    let live = true;
    void api
      .models()
      .then((list) => {
        if (!live) return;
        const ready = list.filter((m) => m.status === "ready");
        setModels(ready);
        setModel((id) =>
          ready.some(
            (m) => m.id === id && m.manifest.task === "person_detector",
          )
            ? id
            : (ready.find((m) => m.manifest.task === "person_detector")?.id ??
              ""),
        );
        setPoseModel((id) =>
          ready.some((m) => m.id === id && m.manifest.task === "pose")
            ? id
            : (ready.find((m) => m.manifest.task === "pose")?.id ?? ""),
        );
      })
      .catch((e) => {
        if (live) setError(message(e));
      });
    return () => {
      live = false;
    };
  }, [terminal]);
  // Restore this frame's detector choice; an old response must never affect a new frame.
  useEffect(() => {
    setPeople(null);
    setChoice(null);
    setPose(null);
    setError("");
    let live = true;
    const intent = modelIntent.current;
    if (frameKey && source && ordinal)
      void api
        .subjectChoice(source, ordinal)
        .then((c) => {
          if (live && c && modelIntent.current === intent) {
            setModel(c.modelId);
            setChoice(c);
          }
        })
        .catch((e) => {
          if (live) setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [frameKey]);
  useEffect(() => {
    const generation = ++request.current;
    let live = true;
    setPeople(null);
    setPose(null);
    if (!frameKey || !source || !ordinal || !model) {
      setReading(false);
      return;
    }
    setReading(true);
    void Promise.all([
      api.personFeature(source, ordinal, model),
      api.subjectChoice(source, ordinal),
    ])
      .then(async ([p, c]) => {
        const chosen =
          p && c?.subject.personFeatureId === p.id
            ? c.subject.personIndex
            : null;
        const result =
          p && chosen !== null && poseModel
            ? await api.poseFeature(p.id, chosen, poseModel)
            : null;
        if (!live || generation !== request.current) return;
        setPeople(p);
        setChoice(c);
        setPose(result);
      })
      .catch((e) => {
        if (live) setError(message(e));
      })
      .finally(() => {
        if (live) setReading(false);
      });
    return () => {
      live = false;
    };
  }, [frameKey, model, poseModel, terminal, refresh]);
  async function choose(index: number | null) {
    if (!people || !source || !ordinal) return;
    const key = frameKey,
      detector = model;
    setSaving(true);
    setError("");
    try {
      const result =
        index === null
          ? (await api.clearSubject(source, ordinal), null)
          : await api.chooseSubject(people.id, index);
      if (currentKey.current === key && currentModel.current === detector) {
        setChoice(result);
        setPose(null);
        setRefresh((n) => n + 1);
      }
    } catch (e) {
      if (currentKey.current === key) setError(message(e));
    } finally {
      setSaving(false);
    }
  }
  async function start(kind: "people" | "pose") {
    if (!source || !ordinal || !frameKey) return;
    const key = frameKey;
    setSaving(true);
    setError("");
    w.setPlaying(false);
    try {
      const result =
        kind === "people"
          ? await api.detectPersons(source, ordinal, model)
          : people && selected !== null
            ? await api.estimatePose(people.id, selected, poseModel)
            : null;
      if (result) {
        setJob(result);
        setTask({ id: result.jobId, frame: key });
      }
    } catch (e) {
      if (currentKey.current === key) setError(message(e));
    } finally {
      setSaving(false);
    }
  }
  return (
    <section className="subject-panel" aria-label="Subject selection">
      <h3>Subject · current frame</h3>
      <label>
        Person detector
        <select
          aria-label="Person detector"
          value={model}
          disabled={saving || running}
          onChange={(e) => {
            modelIntent.current++;
            setModel(e.target.value);
            setError("");
          }}
        >
          <option value="">Choose a model</option>
          {models
            .filter((m) => m.manifest.task === "person_detector")
            .map((m) => (
              <option key={m.id} value={m.id}>
                {m.manifest.name}
              </option>
            ))}
        </select>
      </label>
      {!model && (
        <p className="analysis-note">
          Install a compatible person detector in Settings.
        </p>
      )}
      <Button
        disabled={!model || !frameKey || w.busy || saving || running}
        onClick={() => void start("people")}
      >
        Detect people
      </Button>
      {!frameKey && (
        <p className="analysis-note">Open a frame to choose a subject.</p>
      )}
      {reading && <p role="status">Loading frame results…</p>}
      {people && (
        <>
          <p className="analysis-note">
            {people.result.persons.length
              ? "Choose a person. This frame only."
              : "No people detected. Draw a guided region."}
          </p>
          {people.result.persons.length > 0 && image && (
            <svg
              className="subject-preview"
              viewBox={`0 0 ${image.width} ${image.height}`}
              role="img"
              aria-label="Detected person regions. Use the buttons below to choose a person."
            >
              <image
                href={api.image(image.imagePath)}
                width={image.width}
                height={image.height}
              />
              {people.result.persons.map((p, i) => (
                <g key={i}>
                  <rect
                    x={p.detectorBox[0]}
                    y={p.detectorBox[1]}
                    width={Math.max(0, p.detectorBox[2] - p.detectorBox[0])}
                    height={Math.max(0, p.detectorBox[3] - p.detectorBox[1])}
                    className={selected === i ? "selected" : ""}
                  />
                  <text
                    x={Math.max(
                      0,
                      Math.min(image.width - 30, p.detectorBox[0]),
                    )}
                    y={Math.max(
                      24,
                      Math.min(image.height - 8, p.detectorBox[1] + 24),
                    )}
                    fontSize={Math.max(24, image.width / 20)}
                  >
                    {i + 1}
                  </text>
                </g>
              ))}
            </svg>
          )}
          {people.result.persons.length > 0 && (
            <p className="analysis-note">
              Detector regions locate people; draw guided regions for clothing
              or details.
            </p>
          )}
          <div className="subject-list" aria-label="Detected people">
            {people.result.persons.map((_, i) => (
              <Button
                key={i}
                aria-pressed={selected === i}
                disabled={saving || running || w.busy}
                onClick={() => void choose(i)}
              >
                Person {i + 1}
                {selected === i ? " · Selected" : ""}
              </Button>
            ))}
          </div>
          {selected !== null && (
            <Button
              disabled={saving || running}
              onClick={() => void choose(null)}
            >
              Clear subject
            </Button>
          )}
        </>
      )}
      {selected !== null && (
        <>
          <label>
            Pose model
            <select
              aria-label="Pose model"
              value={poseModel}
              disabled={saving || running}
              onChange={(e) => setPoseModel(e.target.value)}
            >
              <option value="">Choose a model</option>
              {models
                .filter((m) => m.manifest.task === "pose")
                .map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.manifest.name}
                  </option>
                ))}
            </select>
          </label>
          <Button
            disabled={!poseModel || saving || running || w.busy}
            onClick={() => void start("pose")}
          >
            Inspect pose
          </Button>
          <p className="analysis-note">
            Pose inspection is experimental. Automatic visibility scoring is
            unavailable.
          </p>
          {pose?.result.pose && image && (
            <>
              <label className="subject-toggle">
                <input
                  type="checkbox"
                  checked={showMask}
                  onChange={(e) => setShowMask(e.target.checked)}
                />
                Show person mask
              </label>
              <svg
                className="subject-preview"
                viewBox={`0 0 ${image.width} ${image.height}`}
                role="img"
                aria-label="Pose landmark inspection; automatic visibility is unavailable"
              >
                <image
                  href={api.image(image.imagePath)}
                  width={image.width}
                  height={image.height}
                />
                {showMask && pose.result.maskPath && (
                  <image
                    href={api.image(pose.result.maskPath)}
                    width={image.width}
                    height={image.height}
                    opacity=".25"
                  />
                )}
                {pose.result.pose.landmarks.slice(0, 33).map((p, i) => (
                  <circle
                    key={i}
                    cx={p.x}
                    cy={p.y}
                    r={Math.max(2, image.width / 180)}
                  />
                ))}
              </svg>
            </>
          )}
          {pose && (
            <p role="status">
              {pose.result.pose
                ? `${pose.result.pose.landmarks.length} landmarks · person mask available`
                : "No usable pose. Draw or correct a guided region."}
            </p>
          )}
        </>
      )}
      {running && (
        <div className="subject-job" role="status">
          <p>
            {task?.frame !== frameKey ? "Other frame: " : ""}
            {job.status === "cancelling"
              ? "Cancelling…"
              : job.stage === "pose_estimation"
                ? "Inspecting pose…"
                : "Detecting people…"}
          </p>
          <Button
            disabled={job.status === "cancelling"}
            onClick={() =>
              void api.cancel(job.jobId).catch((e) => setError(message(e)))
            }
          >
            Cancel
          </Button>
        </div>
      )}
      {job?.status === "failed" && task?.frame === frameKey && (
        <p role="alert">
          {job.error?.message ??
            "Detection failed. Retry or draw a guided region."}
        </p>
      )}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
