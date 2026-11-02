import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@frameiq/ui";
import { fashionApi } from "../fashionClient";
import { api, message } from "../client";
import { useJobs } from "../store";
import type { FeatureCapabilityV1, JobEvent, TaskPackV2 } from "../contracts";

const packs = [
  {
    task: "detection",
    name: "Item detection",
    model: "GroundingDINO-T",
    size: "695 MB",
    description:
      "Finds clothes, shoes, bags and accessories in photos and video frames. Also helps the Accessories and Footwear presets find detail regions.",
  },
  {
    task: "hair",
    name: "Hair selection",
    model: "MediaPipe hair",
    size: "16.5 MB",
    description:
      "Traces the visible hair area for cleaner hair crops and the Hairstyle preset. You can correct the selection afterward.",
  },
  {
    task: "mask",
    name: "Item cutouts",
    model: "SAM ViT-B",
    size: "375 MB",
    description:
      "Traces an item's edges after you select it, creating a detailed cutout instead of a rectangular crop.",
  },
  {
    task: "embedding",
    name: "Visual matching",
    model: "SigLIP 2 Base",
    size: "372 MB",
    description:
      "Finds similar products in your imported catalog and helps match the same garment across frames. A visual match does not prove product identity.",
  },
];
const size = (bytes: number) =>
  bytes < 1_000_000
    ? `${(bytes / 1_000).toFixed(1)} KB`
    : `${(bytes / 1_000_000).toFixed(1)} MB`;
const running = (job: JobEvent) =>
  ["queued", "running", "cancelling"].includes(job.status);

export function FashionModels({
  onboarding = false,
  onBusyChange,
}: {
  onboarding?: boolean;
  onBusyChange?: (busy: boolean) => void;
}) {
  const [capabilities, setCapabilities] = useState<FeatureCapabilityV1[]>([]);
  const [catalog, setCatalog] = useState<TaskPackV2[]>([]);
  const [pack, setPack] = useState<TaskPackV2 | null>(null);
  const [path, setPath] = useState("");
  const [reviewed, setReviewed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [loadError, setLoadError] = useState("");
  const [job, setJob] = useState<JobEvent | null>(null);
  const setStoredJob = useJobs((s) => s.setJob);
  const reviewTitle = useRef<HTMLHeadingElement>(null);

  async function refresh() {
    setLoading(true);
    setLoadError("");
    try {
      const [capabilities, catalog] = await Promise.all([
        fashionApi.capabilities(),
        fashionApi.modelCatalog(),
      ]);
      setCapabilities(capabilities);
      setCatalog(catalog);
    } catch (e) {
      setLoadError(message(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    void refresh();
  }, []);
  useEffect(() => {
    onBusyChange?.(busy);
  }, [busy, onBusyChange]);
  useEffect(() => {
    reviewTitle.current?.focus();
  }, [pack]);

  async function pick(task: string) {
    setBusy(true);
    setError("");
    try {
      const p = await open({
        multiple: false,
        filters: [{ name: "FrameIQ model package", extensions: ["json"] }],
      });
      if (typeof p !== "string") return;
      const chosen = await fashionApi.readPack(p);
      if (chosen.task !== task || chosen.schemaVersion !== 2)
        throw new Error(
          "This package is for a different feature. Choose the matching package.",
        );
      setPack(chosen);
      setPath(p);
      setReviewed(false);
      setJob(null);
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  async function install() {
    if (!pack || !reviewed || busy) return;
    setBusy(true);
    setError("");
    setJob(null);
    try {
      let current = path
        ? await fashionApi.importPack(path, true)
        : await fashionApi.downloadPack(pack.id, true);
      setJob(current);
      setStoredJob(current);
      while (running(current)) {
        await new Promise((resolve) => setTimeout(resolve, 500));
        const latest = (await api.jobs()).find(
          (j) => j.jobId === current.jobId,
        );
        if (latest) {
          current = latest;
          setJob(current);
          setStoredJob(current);
        }
      }
      if (current.status === "succeeded") {
        setPack(null);
        setReviewed(false);
      } else if (current.status !== "cancelled") {
        setError(
          current.error?.message ??
            "Installation did not complete. Try again or continue without this pack.",
        );
      }
    } catch (e) {
      setError(message(e));
    } finally {
      await refresh();
      setBusy(false);
    }
  }
  async function remove(id: string) {
    setBusy(true);
    setError("");
    setJob(null);
    try {
      await fashionApi.removePack(id);
      await refresh();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section
      className={
        onboarding ? "setup-fashion-models" : "settings-section fashion-models"
      }
      aria-label="Optional fashion packs"
    >
      <div>
        <h2>Optional fashion packs</h2>
        <p>
          Add automatic selections and offline visual matching. Install only
          what you need, or add packs later in Settings → Local models. All four
          use about 1.46 GB of storage.
        </p>
        <p>
          These packs are experimental. ChatGPT item suggestions, descriptions
          and product research do not require them; automatic masks and some
          presets do.
        </p>
        <p>
          Choose a pack to review its download and license. FrameIQ downloads,
          checks and installs it automatically. Interrupted downloads can resume
          when you retry.
        </p>
        {loading && <p role="status">Checking installed fashion packs…</p>}
        {loadError && (
          <div>
            <p role="alert">Could not check installed packs: {loadError}</p>
            <Button disabled={busy} onClick={() => void refresh()}>
              Retry pack status
            </Button>
          </div>
        )}
        {error && !pack && (
          <p role="alert" className="error-banner">
            {error}
          </p>
        )}
        {job?.status === "succeeded" && (
          <p role="status">Pack installed and ready for local analysis.</p>
        )}
        {job?.status === "cancelled" && (
          <p role="status">
            Installation cancelled. You can retry or continue without this pack.
          </p>
        )}
        {packs.map((option) => {
          const capability = capabilities.find((c) => c.task === option.task);
          const download = catalog.find((p) => p.task === option.task);
          const installed =
            !!capability?.modelId &&
            ["experimental", "implemented"].includes(capability.status);
          return (
            <div className="setup-model" key={option.task}>
              <div className="fashion-pack-heading">
                <h3>{option.name}</h3>
                <span>{installed ? "Installed" : `About ${option.size}`}</span>
              </div>
              <p>{option.description}</p>
              <p>{option.model} · Experimental</p>
              <div className="fashion-pack-actions">
                {installed ? (
                  <Button
                    disabled={busy}
                    onClick={() => void remove(capability!.modelId!)}
                    aria-label={`Remove ${option.name} pack`}
                  >
                    Remove pack
                  </Button>
                ) : (
                  <Button
                    disabled={busy || loading || !!loadError || !download}
                    onClick={() => {
                      setPack(download!);
                      setPath("");
                      setReviewed(false);
                      setJob(null);
                      setError("");
                    }}
                    aria-label={`Install ${option.name} pack`}
                  >
                    Install pack
                  </Button>
                )}
              </div>
              {!onboarding && !installed && (
                <details>
                  <summary>Import a local package instead</summary>
                  <Button
                    disabled={busy}
                    onClick={() => void pick(option.task)}
                    aria-label={`Choose local ${option.name} package`}
                  >
                    Choose local package
                  </Button>
                </details>
              )}
              {pack?.task === option.task && (
                <div className="task-package-review">
                  <h3 ref={reviewTitle} tabIndex={-1}>
                    Review installation: {option.name}
                  </h3>
                  <p>
                    {size(
                      pack.artifacts.reduce(
                        (n, a) => n + Number(a.byteSize),
                        0,
                      ),
                    )}{" "}
                    {path ? "installed copy" : "download"} · {pack.license}
                  </p>
                  <p>
                    {path
                      ? "The original package stays where it is. Keep enough free space for both copies."
                      : "Additional temporary storage is needed during installation. Completed download files are removed after the pack is installed."}
                  </p>
                  <details>
                    <summary>Package source, license and checks</summary>
                    <p>
                      {pack.id} · {pack.qualification}
                    </p>
                    <p>License: {pack.licenseUrl}</p>
                    <ul>
                      {pack.artifacts.map((a) => (
                        <li key={a.name}>
                          {a.name} · {size(Number(a.byteSize))}
                          <p>Source: {a.sourceUrl}</p>
                          <p>SHA-256: {a.sha256}</p>
                        </li>
                      ))}
                    </ul>
                  </details>
                  <label>
                    <input
                      type="checkbox"
                      checked={reviewed}
                      disabled={busy}
                      onChange={(e) => setReviewed(e.target.checked)}
                    />{" "}
                    I reviewed this pack's size, license, sources and reference
                    checks.
                  </label>
                  {busy && (
                    <p role="status">
                      {job?.status === "cancelling"
                        ? "Cancelling installation…"
                        : job?.messageCode === "TASK_PACK_DOWNLOADING"
                          ? `Downloading ${size(Number(job.completedUnits))} of ${size(Number(job.totalUnits))}…`
                          : job
                            ? "Installing and checking compatibility…"
                            : "Starting installation…"}
                    </p>
                  )}
                  {busy &&
                    job?.messageCode === "TASK_PACK_DOWNLOADING" &&
                    job.totalUnits && (
                      <progress
                        aria-label={`Downloading ${option.name}`}
                        value={Number(job.completedUnits)}
                        max={Number(job.totalUnits)}
                      />
                    )}
                  {error && (
                    <p role="alert" className="error-banner">
                      {error}
                    </p>
                  )}
                  <div className="fashion-pack-actions">
                    <Button
                      variant="primary"
                      disabled={!reviewed || busy}
                      onClick={() => void install()}
                    >
                      {busy
                        ? "Installing…"
                        : job && job.status !== "succeeded"
                          ? "Retry installation"
                          : path
                            ? "Install reviewed pack"
                            : "Download and install"}
                    </Button>
                    {busy && job && running(job) && (
                      <Button
                        disabled={job.status === "cancelling"}
                        onClick={() =>
                          void api
                            .cancel(job.jobId)
                            .catch((e) => setError(message(e)))
                        }
                      >
                        Cancel installation
                      </Button>
                    )}
                    {!busy && (
                      <Button
                        onClick={() => {
                          setPack(null);
                          setReviewed(false);
                          setError("");
                          setJob(null);
                        }}
                      >
                        Dismiss review
                      </Button>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
