import { CloudProviders } from "./CloudProviders";
import { ModelManager } from "./ModelManager";
import { Button } from "@frameiq/ui";
import { useSettingsTab, usePreferences } from "../store";
import { Film, Keyboard } from "lucide-react";

export function Settings({
  onSetup,
  onTutorial,
}: {
  onSetup?: () => void;
  onTutorial?: () => void;
}) {
  const { tab, setTab } = useSettingsTab();
  const { clearSelectionsOnSwitch, setClearSelectionsOnSwitch } =
    usePreferences();
  return (
    <section className="page settings-page">
      <div className="page-title">
        <div>
          <h1>Settings</h1>
        </div>
      </div>
      <div className="settings-content">
        <nav className="settings-tabs" aria-label="Settings sections">
          <Button
            aria-pressed={tab === "models"}
            onClick={() => setTab("models")}
          >
            Local models
          </Button>
          <Button
            aria-pressed={tab === "cloud"}
            onClick={() => setTab("cloud")}
          >
            Cloud AI
          </Button>
          <Button
            aria-pressed={tab === "general"}
            onClick={() => setTab("general")}
          >
            General & shortcuts
          </Button>
        </nav>
        <div hidden={tab !== "models"}>
          <Button onClick={onSetup}>Run model setup</Button>
          <ModelManager />
        </div>
        <div hidden={tab !== "cloud"}>
          <CloudProviders />
        </div>
        <div hidden={tab !== "general"}>
          <section className="settings-section">
            <div>
              <h2>Selections</h2>
              <label>
                <input
                  type="checkbox"
                  checked={clearSelectionsOnSwitch}
                  onChange={(e) => setClearSelectionsOnSwitch(e.target.checked)}
                />{" "}
                Clear selections when switching files or projects
              </label>
              <p>
                Enabled by default. Clears the outgoing project's frame
                collection when another file or project is opened. Source files
                and existing export plans are kept. Turn this off to collect
                frames across files.
              </p>
              <p>
                Reopening the app or jumping to a selected frame does not clear
                selections.
              </p>
              <Button onClick={onTutorial}>Show workspace tutorial</Button>
            </div>
          </section>
          <p>
            Extraction stays on this computer. Cloud requests require reviewed
            images and explicit approval. Shopping search is unavailable.
          </p>
          <section className="settings-section">
            <Film size={24} />
            <div>
              <h2>Media engine</h2>
              <p>
                FFmpeg 8.0.3 · software reference decode · offline operation.
              </p>
              <p>
                Muted preview may skip display frames; selections use resolved
                presentation ordinals.
              </p>
            </div>
          </section>
          <section className="settings-section">
            <Keyboard size={24} />
            <div>
              <h2>Keyboard controls</h2>
              <dl className="shortcut-list">
                <dt>Previous / next</dt>
                <dd>
                  <kbd>←</kbd>
                  <kbd>→</kbd>
                </dd>
                <dt>Ten frames</dt>
                <dd>
                  <kbd>Shift</kbd> + <kbd>← / →</kbd>
                </dd>
                <dt>Play / pause</dt>
                <dd>
                  <kbd>Space</kbd>
                </dd>
                <dt>Select frame</dt>
                <dd>
                  <kbd>S</kbd>
                </dd>
              </dl>
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}
