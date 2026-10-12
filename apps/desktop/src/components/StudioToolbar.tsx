import { Button } from "@frameiq/ui";
import {
  Check,
  ChevronLeft,
  ChevronRight,
  Pause,
  Play,
  Plus,
  RotateCcw,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { parseTimestamp } from "../client";
import type { Workspace } from "../workspace/useWorkspace";

export function StudioToolbar({
  w,
  cropEditing = false,
}: {
  w: Workspace;
  cropEditing?: boolean;
}) {
  return (
    <div className="studio-toolbar" aria-label="Precision frame controls">
      <div className="playback-controls">
        <Button
          aria-label="Previous frame"
          disabled={!w.frame}
          onClick={() => w.step(-1)}
        >
          <ChevronLeft size={17} />
        </Button>
        <Button
          aria-label={w.playing ? "Pause preview" : "Play muted preview"}
          disabled={!w.frame}
          onClick={() => w.setPlaying((p) => !p)}
        >
          {w.playing ? <Pause size={17} /> : <Play size={17} />}
        </Button>
        <Button
          aria-label="Next frame"
          disabled={!w.frame}
          onClick={() => w.step(1)}
        >
          <ChevronRight size={17} />
        </Button>
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          const t = parseTimestamp(w.timeInput);
          if (t === null) {
            w.setError("Enter a timestamp as HH:MM:SS.mmm or seconds.");
            return;
          }
          w.setPlaying(false);
          void w.seek(t);
        }}
      >
        <input
          className="timestamp"
          aria-label="Seek to timestamp"
          value={w.timeInput}
          onChange={(e) => w.setTimeInput(e.target.value)}
          disabled={!w.activeSource}
        />
      </form>
      <span className="frame-identity">
        {w.frame ? `Frame ${w.frame.ordinal}` : "No frame selected"}
      </span>
      <Button
        variant={w.isSelected ? "secondary" : "primary"}
        disabled={!w.frame || w.busy || w.selectionBusy}
        onClick={() => void w.toggleSelection()}
      >
        {w.isSelected ? <Check size={16} /> : <Plus size={16} />}
        {w.isSelected ? "Selected" : "Select frame"}
      </Button>
      <div className="zoom-controls" aria-label="Image view controls">
        <Button
          aria-label="Zoom out"
          disabled={
            cropEditing ||
            !w.image ||
            w.zoom <= (w.zoomMode === "pixels" ? 0.25 : 1)
          }
          onClick={() =>
            w.setZoom((z) =>
              Math.max(w.zoomMode === "pixels" ? 0.25 : 1, z - 0.5),
            )
          }
        >
          <ZoomOut size={18} />
        </Button>
        <span>
          {w.zoomMode !== "pixels" && w.zoom === 1
            ? "Fit"
            : `${Math.round(w.zoom * 100)}%${w.zoomMode !== "pixels" ? " of fit" : ""}`}
        </span>
        <Button
          aria-label="Actual pixels"
          disabled={cropEditing || !w.image}
          onClick={() => {
            w.setZoomMode("pixels");
            w.setZoom(1);
            w.setPan({ x: 0, y: 0 });
          }}
        >
          1:1
        </Button>
        <Button
          aria-label="Zoom in"
          disabled={cropEditing || !w.image || w.zoom >= 8}
          onClick={() => w.setZoom((z) => Math.min(8, z + 0.5))}
        >
          <ZoomIn size={18} />
        </Button>
        <Button
          aria-label="Reset zoom and pan"
          title="Fit image to viewer"
          disabled={cropEditing || !w.image}
          onClick={() => {
            w.setZoomMode("fit");
            w.setZoom(1);
            w.setPan({ x: 0, y: 0 });
          }}
        >
          <RotateCcw size={18} />
        </Button>
      </div>
    </div>
  );
}
