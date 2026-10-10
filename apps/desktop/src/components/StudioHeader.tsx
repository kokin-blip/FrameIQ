import { Button } from "@frameiq/ui";
import {
  FolderOpen,
  Plus,
  SlidersHorizontal,
  Film,
  Download,
  ChevronRight,
} from "lucide-react";
import type { Workspace } from "../workspace/useWorkspace";

export function StudioHeader({ w }: { w: Workspace }) {
  return (
    <header className="studio-header">
      <button
        className="brand"
        aria-label="FrameIQ project library"
        onClick={() => w.setView("library")}
      >
        FrameIQ
      </button>
      <div className="project-context">
        <button
          className="text-button"
          onClick={() => w.setView("library")}
          aria-label="Projects"
        >
          <FolderOpen size={16} />
          <span>{w.project?.name ?? "Projects"}</span>
        </button>
        {w.view === "extract" && w.sources.length > 0 && (
          <label className="source-picker">
            <ChevronRight size={14} aria-hidden="true" />
            <Film size={14} />
            <select
              aria-label="Source video"
              value={w.sourceId ?? ""}
              onChange={(e) => w.chooseSource(e.target.value)}
            >
              {w.sources.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name} ·{" "}
                  {s.status === "ready" ? `${s.frameCount} frames` : s.status}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
      <nav className="workspace-switch" aria-label="Workspaces">
        <button
          aria-label="Extract frames"
          aria-current={w.view === "extract" ? "page" : undefined}
          className={w.view === "extract" ? "active" : ""}
          onClick={() => w.setView("extract")}
        >
          Studio
        </button>
        <button
          aria-label="Export studio"
          aria-current={w.view === "export" ? "page" : undefined}
          className={w.view === "export" ? "active" : ""}
          onClick={() => w.setView("export")}
        >
          <Download size={14} />
          Export
        </button>
      </nav>
      <Button onClick={() => void w.importVideo()}>
        <Plus size={16} />
        <span>Import video</span>
      </Button>
      <button
        className={w.view === "settings" ? "icon-button active" : "icon-button"}
        aria-label="Settings"
        aria-current={w.view === "settings" ? "page" : undefined}
        onClick={() => w.setView("settings")}
      >
        <SlidersHorizontal size={19} />
      </button>
    </header>
  );
}
