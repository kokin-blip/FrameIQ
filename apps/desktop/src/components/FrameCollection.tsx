import { Button } from "@frameiq/ui";
import { Download, Film } from "lucide-react";
import { timestamp } from "../client";
import { selectionKey } from "../workspace/useFrameThumbnails";
import { Inspector } from "./Inspector";
import type { Workspace } from "../workspace/useWorkspace";

export function FrameCards({
  w,
  images,
}: {
  w: Workspace;
  images: Record<string, string>;
}) {
  return (
    <div className="frame-cards">
      {w.selected.map((s) => {
        const source = w.sources.find((p) => p.id === s.sourceId);
        const active =
          s.sourceId === w.sourceId &&
          s.frameRef.presentationOrdinal === w.frame?.ordinal;
        return (
          <button
            key={selectionKey(s)}
            className={active ? "frame-card selected" : "frame-card"}
            onClick={() => w.openSelection(s)}
            aria-pressed={active}
            aria-label={`Open frame ${s.frameRef.presentationOrdinal} from ${source?.name ?? "source video"}`}
          >
            <div className="frame-card-image">
              {images[selectionKey(s)] ? (
                <img src={images[selectionKey(s)]} alt="" />
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
        );
      })}
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
  return (
    <aside className="frame-collection" aria-label="Selected frame collection">
      <div className="collection-header">
        <h2>Selected frames</h2>
        <span className="count">{w.selected.length}</span>
      </div>
      <div className="selection-list">
        {w.selected.length ? (
          <FrameCards w={w} images={images} />
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
