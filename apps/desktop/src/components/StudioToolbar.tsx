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

export function StudioToolbar({ w }: { w: Workspace }) {
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
        disabled={!w.frame || w.busy}
        onClick={() => void w.toggleSelection()}
      >
        {w.isSelected ? <Check size={16} /> : <Plus size={16} />}
        {w.isSelected ? "Selected" : "Select frame"}
      </Button>
      <div className="zoom-controls" aria-label="Image view controls">
        <Button
          aria-label="Zoom out"
          disabled={!w.image || w.zoom <= 1}
          onClick={() => w.setZoom((z) => Math.max(1, z - 0.5))}
        >
          <ZoomOut size={18} />
        </Button>
        <span>{Math.round(w.zoom * 100)}%</span>
        <Button
          aria-label="Zoom in"
          disabled={!w.image || w.zoom >= 8}
          onClick={() => w.setZoom((z) => Math.min(8, z + 0.5))}
        >
          <ZoomIn size={18} />
        </Button>
        <Button
          aria-label="Reset zoom and pan"
          disabled={!w.image}
          onClick={() => {
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
