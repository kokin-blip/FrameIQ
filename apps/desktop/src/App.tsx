import { useEffect, useState } from "react";
import { api, message } from "./client";
import { useModelRegistry } from "./workspace/modelRegistry";
import { AlertCircle, X } from "lucide-react";
import { useWorkspace } from "./workspace/useWorkspace";
import { useFrameThumbnails } from "./workspace/useFrameThumbnails";
import { StudioHeader } from "./components/StudioHeader";
import { Studio } from "./components/Studio";
import { ProjectLibrary } from "./components/ProjectLibrary";
import { ExportStudio } from "./components/ExportStudio";
import { Settings } from "./components/Settings";
import { Activity } from "./components/Activity";
import { ModelSetup } from "./components/ModelSetup";
import { Tutorial } from "./components/Tutorial";
import { desktop } from "./client";
import { usePreferences } from "./store";

export default function App() {
  const w = useWorkspace();
  const [tutorialActive, setTutorialActive] = useState(false);
  useEffect(() => {
    if (!desktop) return;
    const refresh = () =>
      void useModelRegistry
        .getState()
        .refresh(true)
        .catch((e) => w.setError(message(e)));
    refresh();
    const listener = api.onJob((j) => {
      if (
        j.stage.startsWith("model") &&
        ["succeeded", "failed", "cancelled"].includes(j.status)
      )
        refresh();
    });
    window.addEventListener("focus", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      void listener.then((stop) => stop());
    };
  }, []);
  useEffect(() => {
    if (desktop && w.view === "extract")
      void useModelRegistry
        .getState()
        .refresh(true)
        .catch((e) => w.setError(message(e)));
  }, [w.view]);
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
      {desktop && (
        <Tutorial
          w={w}
          active={tutorialActive}
          onStart={() => setTutorialActive(true)}
        />
      )}
      <main>
        {desktop && (
          <ModelSetup
            visible={w.view === "setup"}
            onFinish={() => {
              usePreferences.getState().completeSetup();
              w.setView("extract");
            }}
          />
        )}
        {w.view === "extract" && <Studio w={w} images={images} />}
        {w.view === "library" && <ProjectLibrary w={w} />}
        {w.view === "export" && <ExportStudio w={w} images={images} />}
        {w.view === "settings" && (
          <Settings
            onSetup={w.openModelSetup}
            onTutorial={() => {
              usePreferences.getState().completeSetup();
              usePreferences.getState().replayTutorial();
              setTutorialActive(true);
              w.setView("extract");
            }}
          />
        )}
      </main>
      <Activity w={w} />
    </div>
  );
}
