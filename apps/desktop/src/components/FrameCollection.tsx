import { Button } from "@frameiq/ui";
import { Download, Film, Plus, Minus, X } from "lucide-react";
import { useState } from "react";
import { timestamp } from "../client";
import {
  selectionKey,
  useFrameThumbnails,
} from "../workspace/useFrameThumbnails";
import { Inspector } from "./Inspector";
import { PresetPanel } from "./PresetPanel";
import type { Selection } from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";

export function FrameCards({
  w,
  images,
  onRemove,
}: {
  w: Workspace;
  images: Record<string, string>;
  onRemove?: (frame: Selection) => void;
}) {
  const [page, setPage] = useState(0);
  const pages = Math.max(1, Math.ceil(w.selected.length / 100));
  const currentPage = Math.min(page, pages - 1);
  const frames = w.selected.slice(currentPage * 100, (currentPage + 1) * 100);
  const pageImages = useFrameThumbnails(currentPage > 0 ? frames : []);
  return (
    <div className="frame-cards">
      {frames.map((s) => {
        const source = w.sources.find((p) => p.id === s.sourceId);
        const active =
          s.sourceId === w.sourceId &&
          s.frameRef.presentationOrdinal === w.frame?.ordinal;
        return (
          <div className="frame-card-row" key={selectionKey(s)}>
            <button
              className={active ? "frame-card selected" : "frame-card"}
              onClick={() => w.openSelection(s)}
              aria-pressed={active}
              aria-label={`Open frame ${s.frameRef.presentationOrdinal} from ${source?.name ?? "source video"}`}
            >
              <div className="frame-card-image">
                {(pageImages[selectionKey(s)] ?? images[selectionKey(s)]) ? (
                  <img
                    src={pageImages[selectionKey(s)] ?? images[selectionKey(s)]}
                    alt=""
                  />
                ) : (
                  <Film size={20} />
                )}
              </div>
              <div>
                <strong>Frame {s.frameRef.presentationOrdinal}</strong>
                <small>{timestamp(s.timeSeconds)}</small>
              </div>
              <span className="frame-source" title={source?.name}>
                {source?.name ?? "Source video"}
              </span>
            </button>
            {onRemove && (
              <Button
                className="frame-remove"
                aria-label={`Remove frame ${s.frameRef.presentationOrdinal} from ${source?.name ?? "source video"}`}
                title="Remove frame"
                disabled={w.selectionBusy}
                onClick={() => onRemove(s)}
              >
                <X size={16} />
              </Button>
            )}
          </div>
        );
      })}
      {pages > 1 && (
        <div className="collection-pagination">
          <Button
            disabled={currentPage === 0}
            onClick={() => setPage(currentPage - 1)}
          >
            Previous
          </Button>
          <span>
            {currentPage + 1} / {pages}
          </span>
          <Button
            disabled={currentPage === pages - 1}
            onClick={() => setPage(currentPage + 1)}
          >
            Next
          </Button>
        </div>
      )}
    </div>
  );
}

export function FrameCollection({
  w,
  images,
}: {
  w: Workspace;
  images: Record<string, string>;
}) {
  const [presetsOpened, setPresetsOpened] = useState(false);
  return (
    <aside className="frame-collection" aria-label="Selected frame collection">
      <div className="collection-header">
        <h2>Selected frames</h2>
        <span className="count">{w.selected.length}</span>
      </div>
      <div className="selection-list">
        <div className="selection-actions" aria-label="Selection actions">
          <Button
            disabled={!w.frame || w.busy || w.selectionBusy}
            onClick={() => void w.toggleSelection()}
          >
            {w.isSelected ? <Minus size={16} /> : <Plus size={16} />}
            {w.isSelected ? "Remove current frame" : "Add current frame"}
          </Button>
          <Button
            disabled={
              !w.activeSource ||
              w.activeSource.status !== "ready" ||
              w.selectionBusy
            }
            title="Select all indexed frames in the current source"
            onClick={() => void w.selectAll().catch(() => {})}
          >
            Select all
          </Button>
          <Button
            disabled={!w.selected.length || w.selectionBusy}
            title="Remove all selected frames across this project"
            onClick={() => void w.clearSelections().catch(() => {})}
          >
            Remove all
          </Button>
        </div>
        {w.selectionBusy && <p role="status">Updating selections…</p>}
        <details
          className="selection-presets"
          onToggle={(e) => {
            if (e.currentTarget.open) setPresetsOpened(true);
          }}
        >
          <summary>Auto-select with a preset</summary>
          {presetsOpened && (
            <PresetPanel key={w.project?.id ?? "no-project"} w={w} />
          )}
        </details>
        {w.selected.length ? (
          <FrameCards
            w={w}
            images={images}
            onRemove={(s) => void w.removeSelection(s).catch(() => {})}
          />
        ) : (
          <p className="collection-empty">
            No frames selected.
            <br />
            Press S or click Select frame.
          </p>
        )}
      </div>
      <Inspector w={w} />
      <div className="collection-footer">
        <Button
          variant="primary"
          aria-label="Export selected"
          disabled={!w.selected.length}
          onClick={() => w.setView("export")}
        >
          <Download size={18} />
          Export {w.selected.length || "selected"}{" "}
          {w.selected.length === 1 ? "frame" : "frames"}
        </Button>
      </div>
    </aside>
  );
}
