import { useEffect, useState } from "react";
import { api, message } from "../client";
import { useJobs } from "../store";
import type { ExportPlanV1 } from "../contracts";
import { Button } from "@frameiq/ui";
import { ArrowLeft, Download, Check, FileImage } from "lucide-react";
import type { Workspace } from "../workspace/useWorkspace";
import type { ColorMode, ExportFormat } from "../contracts";
import { FrameCards } from "./FrameCollection";

export function ExportStudio({
  w,
  images,
}: {
  w: Workspace;
  images: Record<string, string>;
}) {
  const [plans, setPlans] = useState<ExportPlanV1[]>([]);
  const jobs = useJobs((s) => s.jobs);
  const setJob = useJobs((s) => s.setJob);
  const order = useJobs((s) => s.order);
  const projectPlans = plans.filter((p) => p.projectId === w.project?.id);
  const lastJob = order
    .map((id) => jobs[id])
    .find(
      (job) =>
        job.stage === "exporting" &&
        projectPlans.some(
          (plan) =>
            plan.id === job.jobId || job.resultIds.includes(plan.destination),
        ),
    );
  useEffect(() => {
    let alive = true;
    if (!w.project) {
      setPlans([]);
      return;
    }
    api
      .exportPlans(w.project.id)
      .then((p) => {
        if (alive) setPlans(p);
      })
      .catch((e) => w.setError(message(e)));
    return () => {
      alive = false;
    };
  }, [w.project?.id, jobs]);
  async function recover(plan: ExportPlanV1, dismiss = false) {
    try {
      if (dismiss) {
        await api.dismissExport(plan.id);
        setPlans((p) => p.filter((v) => v.id !== plan.id));
      } else setJob(await api.resumeExport(plan.id));
    } catch (e) {
      w.setError(message(e));
    }
  }
  return (
    <section className="page export-page">
      <div className="export-scroll">
        <div className="page-title">
          <div>
            <h1>Export studio</h1>
            <p>Export selected frames at their original resolution.</p>
          </div>
          <Button onClick={() => w.setView("extract")}>
            <ArrowLeft size={15} />
            Back to studio
          </Button>
        </div>
        <div className="export-layout">
          <div className="export-configuration">
            <div className="export-summary">
              <FileImage size={26} />
              <div>
                <strong>
                  {w.selected.length} selected{" "}
                  {w.selected.length === 1 ? "frame" : "frames"}
                </strong>
                <span>Frames from all selected sources</span>
              </div>
            </div>
            <label>
              Image format
              <select
                value={w.format}
                onChange={(e) => w.setFormat(e.target.value as ExportFormat)}
              >
                <option value="png">PNG · lossless</option>
                <option value="jpeg">JPEG</option>
                <option value="webp">WebP · lossless</option>
              </select>
            </label>
            {w.format === "jpeg" && (
              <label>
                JPEG quality
                <input
                  type="number"
                  min={1}
                  max={100}
                  value={w.quality}
                  onChange={(e) => w.setQuality(Number(e.target.value))}
                />
              </label>
            )}
            <p className="fidelity-note">
              <Check size={16} />
              Original resolution · sidecars included
            </p>
            <details
              className="advanced-options"
              open={
                w.colorMode === "preserve16_bit" ||
                !!w.sourceColor ||
                w.sources.some((s) =>
                  s.metadata?.streams.some(
                    (v) =>
                      v.colorTransfer === "smpte2084" ||
                      v.colorTransfer === "arib-std-b67",
                  ),
                ) ||
                undefined
              }
            >
              <summary>Advanced color settings</summary>
              <label>
                Color output
                <select
                  value={w.colorMode}
                  onChange={(e) => w.setColorMode(e.target.value as ColorMode)}
                >
                  <option value="sdr">SDR · sRGB</option>
                  <option value="preserve16_bit" disabled={w.format !== "png"}>
                    Preserve 16-bit · PNG
                  </option>
                </select>
              </label>
              <label>
                Source colors
                <select
                  value={w.sourceColor}
                  onChange={(e) => w.setSourceColor(e.target.value)}
                >
                  <option value="">Use recorded metadata</option>
                  <option value="bt709">Interpret as BT.709</option>
                  <option value="bt601_625">Interpret as BT.601 PAL</option>
                  <option value="bt601_525">Interpret as BT.601 NTSC</option>
                  <option value="hdr10">Interpret as HDR10 / PQ</option>
                  <option value="hlg">Interpret as HLG</option>
                </select>
              </label>
              {w.colorMode === "preserve16_bit" && w.format !== "png" && (
                <p role="alert">Choose PNG for 16-bit output.</p>
              )}
              <p>
                SDR output tone-maps supported HDR sources. PNG16 preserves
                source transfer and primaries. Unknown color metadata requires
                an explicit interpretation.
              </p>
            </details>
          </div>
          <div className="export-frames">
            <h2>Selected frames</h2>
            {w.selected.length ? (
              <FrameCards w={w} images={images} />
            ) : (
              <div className="export-empty">
                <FileImage size={28} />
                <p>No frames selected. Select frames in the studio.</p>
                <Button onClick={() => w.setView("extract")}>
                  Open studio
                </Button>
              </div>
            )}
          </div>
        </div>
        {projectPlans.map((plan) => (
          <section
            className="export-outcome"
            key={plan.id}
            aria-label="Saved export"
          >
            <h2>
              {plan.items.every((i) => i.status === "succeeded")
                ? "Saved export"
                : "Interrupted export"}
            </h2>
            <code>{plan.destination}</code>
            <p>
              {plan.items.filter((i) => i.status === "succeeded").length}{" "}
              completed ·{" "}
              {plan.items.filter((i) => i.status !== "succeeded").length}{" "}
              remaining
            </p>
            {plan.items.map((i) => (
              <p key={i.imagePath}>
                Frame {i.frame.frameRef.presentationOrdinal} · {i.status}
                {i.error ? ` — ${i.error.message}` : ""}
              </p>
            ))}
            <Button disabled={plan.active} onClick={() => void recover(plan)}>
              Resume / verify outputs
            </Button>
            <Button
              disabled={plan.active}
              onClick={() => void recover(plan, true)}
            >
              Dismiss
            </Button>
          </section>
        ))}
        {lastJob && (
          <section className="export-outcome" aria-label="Export outcome">
            <h2>
              {lastJob.status === "succeeded"
                ? "Export complete"
                : "Export status"}
            </h2>
            <p>
              {lastJob.completedUnits} of {lastJob.totalUnits ?? "?"} frames ·{" "}
              {lastJob.status}
            </p>
            {lastJob.resultIds.map((path) => (
              <code key={path}>{path}</code>
            ))}
            {lastJob.error && <p role="alert">{lastJob.error.message}</p>}
          </section>
        )}
      </div>
      <div className="export-action-bar">
        <span>
          {w.selected.length} selected frames · {w.format.toUpperCase()}
        </span>
        <Button
          variant="primary"
          disabled={
            !w.selected.length ||
            w.activeJobs.some((j) => j.stage === "exporting")
          }
          onClick={() => void w.exportFrames()}
        >
          <Download size={16} />
          Choose folder and export
        </Button>
      </div>
    </section>
  );
}
