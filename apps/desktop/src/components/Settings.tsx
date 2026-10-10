import { ModelManager } from "./ModelManager";
import { Film, ShieldCheck, Keyboard } from "lucide-react";

export function Settings() {
  return (
    <section className="page settings-page">
      <div className="page-title">
        <div>
          <h1>Settings and models</h1>
          <p>FrameIQ processes videos on your computer.</p>
        </div>
      </div>
      <div className="settings-content">
        <section className="settings-section">
          <ShieldCheck size={24} />
          <div>
            <h2>External search disabled</h2>
            <p>
              Project images stay on this computer. Model downloads and optional
              loopback compatibility tests require your action.
            </p>
            <span className="status-badge">Offline by design</span>
          </div>
        </section>
        <ModelManager />
        <section className="settings-section">
          <Film size={24} />
          <div>
            <h2>Media engine</h2>
            <p>FFmpeg 8.0.3 · software reference decode · offline operation.</p>
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
    </section>
  );
}
