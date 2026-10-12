import { useEffect, useRef, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, message, timestamp } from "../client";
import { useSettingsTab } from "../store";
import { useJobs } from "../store";
import type {
  CloudProviderV1,
  CloudTaskV1,
  CloudReviewV1,
  CloudAnalysisResultV1,
  RegionRectV1,
  Selection,
} from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";
const frameKey = (f: Selection) =>
  `${f.sourceId}:${f.frameRef.indexGeneration}:${f.frameRef.presentationOrdinal}`;
const goals = [
  "general",
  "makeup",
  "hairstyle",
  "outfit",
  "complete_look",
  "accessories",
  "footwear",
  "custom_region",
];
export function CloudAnalysisPanel({
  w,
  region,
  onCropChange,
}: {
  w: Workspace;
  region?: [RegionRectV1, React.Dispatch<React.SetStateAction<RegionRectV1>>];
  onCropChange?: (active: boolean) => void;
}) {
  const [provider, setProvider] = useState<CloudProviderV1>("openai");
  const [model, setModel] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [saved, setSaved] = useState(false);
  const [task, setTask] = useState<CloudTaskV1>("describe");
  const [goal, setGoal] = useState("outfit");
  const [chosen, setChosen] = useState<string[]>([]);
  const [crop, setCrop] = useState(false);
  const [internalRectangle, setInternalRectangle] = useState<RegionRectV1>({
    x: 0,
    y: 0,
    width: 1,
    height: 1,
  });
  const rectangle = region?.[0] ?? internalRectangle;
  const setRectangle = region?.[1] ?? setInternalRectangle;
  useEffect(() => {
    onCropChange?.(crop);
    return () => onCropChange?.(false);
  }, [crop, onCropChange]);
  const [review, setReview] = useState<CloudReviewV1 | null>(null);
  const [loaded, setLoaded] = useState<string[]>([]);
  const [consent, setConsent] = useState(false);
  const [jobId, setJobId] = useState<string>();
  const [result, setResult] = useState<CloudAnalysisResultV1 | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const generation = useRef(0);
  const preparedContext = useRef("");
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob);
  const active = jobId ? jobs[jobId] : undefined;
  const running =
    busy ||
    (!!active && ["queued", "running", "cancelling"].includes(active.status));
  const selected = w.selected.filter((f) => chosen.includes(frameKey(f)));
  const current =
    w.image && w.sourceId
      ? {
          sourceId: w.sourceId,
          frameRef: w.image.frameRef,
          timeSeconds: w.image.timeSeconds,
        }
      : null;
  const context = JSON.stringify([
    provider,
    model,
    task,
    goal,
    task === "describe" ? current?.frameRef : selected.map(frameKey),
    crop,
    rectangle,
  ]);
  useEffect(() => {
    let live = true;
    setSaved(false);
    setModels([]);
    setModel("");
    void api
      .cloudKeyStatus(provider)
      .then((s) => {
        if (live) setSaved(s.saved);
      })
      .catch((e) => {
        if (live) setError(message(e));
      });
    return () => {
      live = false;
    };
  }, [provider]);
  useEffect(() => {
    generation.current++;
    setReview(null);
    setConsent(false);
    setLoaded([]);
  }, [context]);
  useEffect(
    () => () => {
      if (review) void api.discardCloudReview(review.id).catch(() => {});
    },
    [review?.id],
  );
  useEffect(() => {
    let live = true;
    if (
      !active ||
      !["succeeded", "failed", "cancelled"].includes(active.status)
    )
      return;
    if (active.status !== "succeeded") {
      if (active.error) setError(active.error.message);
      return;
    }
    if (
      active.stage === "cloud_preparing" &&
      preparedContext.current !== context
    ) {
      void api.discardCloudReview(active.jobId).catch(() => {});
      return;
    }
    if (active.stage === "cloud_preparing")
      void api
        .cloudReview(active.jobId)
        .then((r) => {
          if (live) {
            setReview(r);
            setLoaded([]);
            setConsent(false);
          }
        })
        .catch((e) => {
          if (live) setError(message(e));
        });
    if (active.stage === "cloud_analysis")
      void api
        .cloudResult(active.jobId)
        .then((r) => {
          if (live) {
            setResult(r);
            if (!r) setError("Suggestions are no longer cached.");
          }
        })
        .catch((e) => {
          if (live) setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [active?.jobId, active?.status]);
  async function prepare() {
    const frames = task === "describe" ? (current ? [current] : []) : selected;
    if (!frames.length) return;
    const token = generation.current;
    setBusy(true);
    setError("");
    setResult(null);
    try {
      const job = await api.prepareCloudReview({
        provider,
        model,
        task,
        goal,
        images: frames.map((frame) => ({
          frame,
          region: task === "describe" && crop ? rectangle : null,
        })),
      });
      setJob(job);
      if (generation.current === token) {
        preparedContext.current = context;
        setJobId(job.jobId);
      } else
        void api
          .cancel(job.jobId)
          .catch(() => {})
          .finally(() => api.discardCloudReview(job.jobId).catch(() => {}));
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  async function send() {
    if (!review || !consent || loaded.length !== review.images.length) return;
    setBusy(true);
    setError("");
    try {
      const job = await api.analyzeCloudReview(review.id, true);
      setJob(job);
      setJobId(job.jobId);
      setReview(null);
      setConsent(false);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  async function listModels() {
    setBusy(true);
    setError("");
    try {
      const r = await api.testCloudConnection(provider);
      setModels(r.models);
      if (r.moreAvailable)
        setError(
          "This is the first model-list page. You can also enter a model ID.",
        );
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  const name = provider === "openai" ? "OpenAI" : "Gemini";
  return (
    <aside
      className="analysis-panel cloud-analysis"
      aria-label="Cloud AI analysis"
    >
      <h2>Cloud AI</h2>
      <p>
        Describe a frame or region, or rank chosen frames for a preset.
        Suggestions are unverified.
      </p>
      <fieldset disabled={running}>
        <legend>Request</legend>
        <label>
          Provider
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as CloudProviderV1)}
          >
            <option value="openai">OpenAI</option>
            <option value="gemini">Gemini</option>
          </select>
        </label>
        {!saved && (
          <>
            <p>No {name} key saved.</p>
            <Button
              onClick={() => {
                useSettingsTab.getState().setTab("cloud");
                w.setView("settings");
              }}
            >
              Open Settings
            </Button>
          </>
        )}
        <Button disabled={!saved} onClick={() => void listModels()}>
          List provider models
        </Button>
        {models.length > 0 && (
          <label>
            Provider model
            <select
              value={models.includes(model) ? model : ""}
              onChange={(e) => setModel(e.target.value)}
            >
              <option value="">Choose a model</option>
              {models.map((id) => (
                <option key={id} value={id}>
                  {id}
                </option>
              ))}
            </select>
          </label>
        )}
        <p>Model lists do not confirm image or structured-output support.</p>
        <details open={!model || undefined}>
          <summary>Model ID / advanced</summary>
          <label>
            Image-capable model ID
            <input
              value={model}
              maxLength={128}
              onChange={(e) => setModel(e.target.value)}
            />
          </label>
        </details>
        <label>
          Task
          <select
            value={task}
            onChange={(e) => setTask(e.target.value as CloudTaskV1)}
          >
            <option value="describe">Describe current frame or region</option>
            <option value="rank">Rank chosen frames</option>
          </select>
        </label>
        <label>
          Preset goal
          <select value={goal} onChange={(e) => setGoal(e.target.value)}>
            {goals.map((g) => (
              <option value={g} key={g}>
                {g.replaceAll("_", " ")}
              </option>
            ))}
          </select>
        </label>
        {task === "describe" ? (
          <>
            <label className="check-row">
              <input
                type="checkbox"
                checked={crop}
                onChange={(e) => setCrop(e.target.checked)}
              />
              Crop the current frame
            </label>
            {crop && (
              <>
                <p>
                  Draw on the main frame. Drag the crop to move it or its
                  lower-right corner to resize.
                </p>
                <div className="cloud-region">
                  {(["x", "y", "width", "height"] as const).map((k) => (
                    <label key={k}>
                      {k} (%)
                      <input
                        type="number"
                        min={0}
                        max={100}
                        step={1}
                        value={rectangle[k] * 100}
                        onChange={(e) =>
                          setRectangle((r) => ({
                            ...r,
                            [k]: Number(e.target.value) / 100,
                          }))
                        }
                      />
                    </label>
                  ))}
                </div>
              </>
            )}
          </>
        ) : (
          <>
            <p>Choose 2–8 saved frames ({selected.length} chosen).</p>
            <div className="cloud-candidates">
              {w.selected.map((f) => (
                <label className="check-row" key={frameKey(f)}>
                  <input
                    type="checkbox"
                    checked={chosen.includes(frameKey(f))}
                    disabled={
                      !chosen.includes(frameKey(f)) && selected.length >= 8
                    }
                    onChange={(e) =>
                      setChosen((v) =>
                        e.target.checked
                          ? [...v, frameKey(f)]
                          : v.filter((k) => k !== frameKey(f)),
                      )
                    }
                  />
                  {w.sources.find((s) => s.id === f.sourceId)?.name ?? "Source"}{" "}
                  · {timestamp(f.timeSeconds)}
                </label>
              ))}
            </div>
          </>
        )}
        <Button
          variant="primary"
          disabled={
            !saved ||
            !model.trim() ||
            (task === "describe"
              ? !current
              : selected.length < 2 || selected.length > 8)
          }
          onClick={() => void prepare()}
        >
          Review images
        </Button>
      </fieldset>
      {running && (
        <div role="status">
          <p>
            {active?.stage === "cloud_preparing"
              ? `Preparing ${active.completedUnits}/${active.totalUnits ?? "?"} images locally…`
              : "Cloud request pending…"}
          </p>
          {jobId && (
            <Button
              disabled={active?.status === "cancelling"}
              onClick={() =>
                void api.cancel(jobId).catch((e) => setError(message(e)))
              }
            >
              Cancel
            </Button>
          )}
          <p>Sent data cannot be recalled. A request may already be billed.</p>
        </div>
      )}
      {review && (
        <section aria-label="Review cloud upload">
          <h3>
            Review images for{" "}
            {review.provider === "openai" ? "OpenAI" : "Gemini"}
          </h3>
          <p>
            {review.model} · {review.goal.replaceAll("_", " ")}
          </p>
          <p>
            These exact JPEG images will be sent, up to 1024 pixels per side.
            Metadata is removed. Faces and backgrounds remain if visible.
          </p>
          {review.images.map((image) => (
            <figure key={image.id}>
              <img
                src={api.image(image.path)}
                alt={`Upload preview ${image.id}`}
                onLoad={() =>
                  setLoaded((v) =>
                    v.includes(image.id) ? v : [...v, image.id],
                  )
                }
                onError={() =>
                  setError(
                    "Upload preview could not load. Prepare the images again.",
                  )
                }
              />
              <figcaption>
                {timestamp(image.input.frame.timeSeconds)} · {image.width}×
                {image.height} · {Math.ceil(image.bytes / 1024)} KiB
              </figcaption>
            </figure>
          ))}
          <p>
            Provider charges and data-use terms apply. One request; no automatic
            retry. Maximum output:{" "}
            {review.images.length === 1 ? "1,200" : "3,000"} tokens. No price
            estimate is available.
          </p>
          <label className="check-row">
            <input
              type="checkbox"
              checked={consent}
              disabled={loaded.length !== review.images.length}
              onChange={(e) => setConsent(e.target.checked)}
            />
            Allow sending these images to {name} using my provider account.
          </label>
          <Button
            variant="primary"
            disabled={
              !consent || loaded.length !== review.images.length || running
            }
            onClick={() => void send()}
          >
            Send to {name}
          </Button>
          <Button disabled={running} onClick={() => setReview(null)}>
            Discard review
          </Button>
        </section>
      )}
      {error && (
        <p className="cloud-error" role="alert">
          {error}
        </p>
      )}
      {result && (
        <section aria-label="Cloud AI suggestions">
          <h3>AI suggestions · unverified</h3>
          <p>
            {result.actualModel ?? result.review.model} ·{" "}
            {result.review.goal.replaceAll("_", " ")}
          </p>
          {result.inputTokens !== null && (
            <p>
              {result.inputTokens} input tokens ·{" "}
              {result.outputTokens ?? "unknown"} output tokens
            </p>
          )}
          {result.assessments.map((a) => {
            const image = result.review.images.find((i) => i.id === a.imageId)!;
            return (
              <article key={a.imageId}>
                <h4>
                  {a.rank}. {timestamp(image.input.frame.timeSeconds)}
                </h4>
                <img
                  src={api.image(image.path)}
                  alt={`Analyzed ${a.imageId}`}
                />
                <p>{a.reason}</p>
                <h4>Observed by AI</h4>
                <ul>
                  {a.observed.map((v, i) => (
                    <li key={i}>{v}</li>
                  ))}
                </ul>
                {a.uncertain.length > 0 && (
                  <>
                    <h4>Uncertain</h4>
                    <ul>
                      {a.uncertain.map((v, i) => (
                        <li key={i}>{v}</li>
                      ))}
                    </ul>
                  </>
                )}
                <Button onClick={() => void w.openSelection(image.input.frame)}>
                  Open exact frame
                </Button>
                <Button
                  onClick={() =>
                    void api
                      .select(
                        image.input.frame.sourceId,
                        image.input.frame.frameRef.presentationOrdinal,
                        true,
                      )
                      .catch((e) => setError(message(e)))
                  }
                >
                  Add frame
                </Button>
              </article>
            );
          })}
          <p>
            Local measurements and rankings were not changed. AI descriptions
            can be wrong; verify them in the frame.
          </p>
        </section>
      )}
    </aside>
  );
}
