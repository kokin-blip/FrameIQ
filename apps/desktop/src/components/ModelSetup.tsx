import { useEffect, useRef, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, message } from "../client";
import { useJobs } from "../store";
import type {
  HardwareProfileV1,
  JobEvent,
  ModelManifestV1,
} from "../contracts";

const purpose: Record<string, string> = {
  person_detector: "Find and choose a person in the current frame.",
  pose: "Body landmarks and a person mask. Requires the person detector.",
  face_detector: "Face regions and landmarks for makeup detail.",
  face: "Face regions and landmarks for makeup detail.",
  yunet: "Face regions and landmarks for makeup detail.",
};
const size = (bytes: string | number) =>
  `${(Number(bytes) / 1_000_000).toFixed(1)} MB`;
type Outcome = { status: string; message?: string; job?: JobEvent };

export function ModelSetup({
  visible,
  onFinish,
}: {
  visible: boolean;
  onFinish: () => void;
}) {
  const [catalog, setCatalog] = useState<ModelManifestV1[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  const [installed, setInstalled] = useState<string[]>([]);
  const [hardware, setHardware] = useState<HardwareProfileV1>();
  const [step, setStep] = useState<"choose" | "review" | "install">("choose");
  const [reviewed, setReviewed] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadKey, setLoadKey] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [outcomes, setOutcomes] = useState<Record<string, Outcome>>({});
  const cancel = useRef(false),
    activeJob = useRef<string | undefined>(undefined);
  const setJob = useJobs((s) => s.setJob);
  const selected = catalog.filter((m) => chosen.includes(m.id));
  const pending = selected.filter((m) => !installed.includes(m.id));
  const detector = catalog.find((m) => m.task === "person_detector");
  useEffect(() => {
    if (!visible || busy) return;
    let live = true;
    setStep("choose");
    setReviewed(false);
    setError("");
    setLoading(true);
    void Promise.all([api.modelCatalog(), api.models(), api.hardware()])
      .then(([packs, models, h]) => {
        if (!live) return;
        const ready = models
          .filter((m) => m.status === "ready")
          .map((m) => m.manifest.id);
        setCatalog(packs);
        setInstalled(ready);
        setHardware(h);
        setChosen(packs.filter((m) => !ready.includes(m.id)).map((m) => m.id));
      })
      .catch((e) => {
        if (live) setError(message(e));
      })
      .finally(() => {
        if (live) setLoading(false);
      });
    return () => {
      live = false;
    };
  }, [visible, loadKey]);

  async function install() {
    if (!reviewed || busy) return;
    cancel.current = false;
    setBusy(true);
    setError("");
    setStep("install");
    try {
      // Recheck installations before retrying; never redownload successful packs.
      const ready = (await api.models())
        .filter((m) => m.status === "ready")
        .map((m) => m.manifest.id);
      setInstalled(ready);
      for (const pack of selected.filter((m) => !ready.includes(m.id))) {
        if (cancel.current) break;
        try {
          const first = await api.downloadModel(pack);
          activeJob.current = first.jobId;
          setJob(first);
          if (cancel.current) await api.cancel(first.jobId);
          let job = first;
          while (["queued", "running", "cancelling"].includes(job.status)) {
            setOutcomes((s) => ({
              ...s,
              [pack.id]: { status: job.status, job },
            }));
            await new Promise((resolve) => setTimeout(resolve, 500));
            const found = (await api.jobs()).find(
              (j) => j.jobId === first.jobId,
            );
            if (found) {
              job = found;
              setJob(job);
            }
          }
          setOutcomes((s) => ({
            ...s,
            [pack.id]: {
              status: job.status,
              job,
              message: job.error?.message,
            },
          }));
          if (job.status === "succeeded") {
            ready.push(pack.id);
            setInstalled([...ready]);
          }
        } catch (e) {
          setOutcomes((s) => ({
            ...s,
            [pack.id]: { status: "failed", message: message(e) },
          }));
          // A disconnected command channel must not start additional downloads.
          break;
        } finally {
          activeJob.current = undefined;
        }
      }
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  async function stop() {
    cancel.current = true;
    try {
      if (activeJob.current) await api.cancel(activeJob.current);
    } catch (e) {
      setError(message(e));
    }
  }

  return (
    <section
      className="page model-setup"
      hidden={!visible}
      aria-label="Model setup"
    >
      <div className="setup-content">
        <h1>
          {step === "choose"
            ? "Set up local models"
            : step === "review"
              ? "Review downloads"
              : "Model installation"}
        </h1>
        <p>
          Frame extraction works without models. Choose models for optional
          local analysis.
        </p>
        <p>Choose models → Review downloads → Install</p>
        {hardware && (
          <p>
            {hardware.architecture} ·{" "}
            {(Number(hardware.totalMemoryBytes) / 1073741824).toFixed(0)} GiB
            RAM · {(Number(hardware.aiBudgetBytes) / 1073741824).toFixed(1)} GiB
            managed AI budget. Inference still requires memory and compatibility
            checks.
          </p>
        )}
        {loading && <p role="status">Loading available models…</p>}
        {error && (
          <div>
            <p role="alert" className="error-banner">
              {error}
            </p>
            {!busy && (
              <Button onClick={() => setLoadKey((n) => n + 1)}>
                Reload available models
              </Button>
            )}
          </div>
        )}
        {step === "choose" &&
          catalog.map((m) => {
            const ready = installed.includes(m.id);
            const needsPerson =
              m.task === "pose" &&
              detector &&
              !chosen.includes(detector.id) &&
              !installed.includes(detector.id);
            return (
              <div className="setup-model" key={m.id}>
                <label>
                  <input
                    type="checkbox"
                    checked={ready || chosen.includes(m.id)}
                    disabled={ready || !!needsPerson}
                    onChange={(e) => {
                      setReviewed(false);
                      setChosen((ids) =>
                        e.target.checked
                          ? [...ids, m.id]
                          : ids.filter(
                              (id) =>
                                id !== m.id &&
                                !(
                                  m.task === "person_detector" &&
                                  catalog.find((p) => p.id === id)?.task ===
                                    "pose"
                                ),
                            ),
                      );
                    }}
                  />{" "}
                  {m.name} · {size(m.byteSize)}
                  {ready ? " · installed" : ""}
                </label>
                <p>{purpose[m.task] ?? "Optional local analysis model."}</p>
                {needsPerson && <p>Select the person detector first.</p>}
              </div>
            );
          })}
        {step === "review" && (
          <>
            <p>
              {pending.length} downloads ·{" "}
              {size(pending.reduce((n, m) => n + Number(m.byteSize), 0))} total.
              Downloads use pinned HTTPS sources; project images stay local.
            </p>
            {pending.map((m) => (
              <section className="setup-model" key={m.id}>
                <h2>{m.name}</h2>
                <p>
                  {size(m.byteSize)} · {m.license}
                </p>
                <details>
                  <summary>Source, license, and checksum</summary>
                  <p>Source: {m.sourceUrl}</p>
                  <p>License: {m.licenseUrl}</p>
                  <p>Exact bytes: {m.byteSize}</p>
                  <p>SHA-256: {m.sha256}</p>
                </details>
              </section>
            ))}
            <label>
              <input
                type="checkbox"
                checked={reviewed}
                onChange={(e) => setReviewed(e.target.checked)}
              />{" "}
              I reviewed the selected model sources, sizes, licenses, and
              checksums.
            </label>
          </>
        )}
        {step === "install" && (
          <div aria-live="polite">
            {selected.map((m) => {
              const o = outcomes[m.id];
              return (
                <div className="setup-model" key={m.id}>
                  <strong>{m.name}</strong>
                  <p>
                    {installed.includes(m.id)
                      ? "Installed and compatibility checked"
                      : (o?.status ??
                        (cancel.current ? "Not installed" : "Waiting"))}
                  </p>
                  {o?.job &&
                    ["queued", "running", "cancelling"].includes(o.status) && (
                      <p>
                        {o.job.stage} · {o.job.completedUnits} /{" "}
                        {o.job.totalUnits ?? "unknown"} units
                      </p>
                    )}
                  {o?.message && <p role="alert">{o.message}</p>}
                </div>
              );
            })}
          </div>
        )}
        <p>
          These model packs and automatic presets are experimental.
          Compatibility checks do not establish accuracy. Hair, accessory, and
          footwear automatic models are not available yet.
        </p>
        <p>
          Installed vision runtimes and cloud AI keys can be connected later in
          Settings.
        </p>
        <div className="setup-actions">
          {step === "choose" && (
            <Button
              variant="primary"
              disabled={loading || pending.length === 0}
              onClick={() => setStep("review")}
            >
              Review downloads
            </Button>
          )}
          {step === "review" && (
            <>
              <Button disabled={busy} onClick={() => setStep("choose")}>
                Back
              </Button>
              <Button
                variant="primary"
                disabled={!reviewed || !pending.length || busy}
                onClick={() => void install()}
              >
                Download selected models
              </Button>
            </>
          )}
          {step === "install" && busy && (
            <Button onClick={() => void stop()}>Cancel downloads</Button>
          )}
          {step === "install" && !busy && pending.length > 0 && (
            <Button onClick={() => void install()}>
              Retry unfinished models
            </Button>
          )}
          <Button disabled={busy} onClick={onFinish}>
            {step === "install" || pending.length === 0
              ? "Continue to tutorial"
              : "Skip model setup"}
          </Button>
        </div>
      </div>
    </section>
  );
}
