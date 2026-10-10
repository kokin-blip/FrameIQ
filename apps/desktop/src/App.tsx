import { AlertCircle, X } from "lucide-react";
import { useWorkspace } from "./workspace/useWorkspace";
import { useFrameThumbnails } from "./workspace/useFrameThumbnails";
import { StudioHeader } from "./components/StudioHeader";
import { Studio } from "./components/Studio";
import { ProjectLibrary } from "./components/ProjectLibrary";
import { ExportStudio } from "./components/ExportStudio";
import { Settings } from "./components/Settings";
import { Activity } from "./components/Activity";

export default function App() {
  const w = useWorkspace();
  const images = useFrameThumbnails(w.selected);
  return (
    <div className="app-shell">
      <StudioHeader w={w} />
      {w.error && (
        <div className="error-banner" role="alert">
          <AlertCircle size={17} />
          <span>{w.error}</span>
          <button
            className="icon-button"
            aria-label="Dismiss error"
            onClick={() => w.setError("")}
          >
            <X size={16} />
          </button>
        </div>
      )}
      <main>
        {w.view === "extract" && <Studio w={w} images={images} />}
        {w.view === "library" && <ProjectLibrary w={w} />}
        {w.view === "export" && <ExportStudio w={w} images={images} />}
        {w.view === "settings" && <Settings />}
      </main>
      <Activity w={w} />
    </div>
  );
}
