import { useEffect, useState } from "react";
import { Button } from "@frameiq/ui";
import { usePreferences } from "../store";
import type { Workspace } from "../workspace/useWorkspace";

export function Tutorial({ w }: { w: Workspace }) {
  const { setupCompleted, tutorialCompleted, completeTutorial } =
    usePreferences();
  const [step, setStep] = useState(0);
  useEffect(() => {
    if (tutorialCompleted) setStep(0);
  }, [tutorialCompleted]);
  if (
    !setupCompleted ||
    tutorialCompleted ||
    !["extract", "export"].includes(w.view)
  )
    return null;
  const titles = ["Attach a video", "Choose frames", "Export selected frames"];
  const copy = [
    "Drop a video on the viewer or click Import video. If a video is already open, continue with it.",
    "Use ← / → or the precision wheel to step frames. Enter a timestamp to jump. Press S or click Select frame to save it. Expand precision to inspect neighboring frames.",
    "Choose PNG, JPEG, or WebP in Export Studio, then choose a folder. Images keep their original resolution and include provenance sidecars.",
  ];
  return (
    <section className="tutorial-bar" aria-label="Workspace tutorial">
      <div>
        <strong>
          {step + 1} of 3 · {titles[step]}
        </strong>
        <p>{copy[step]}</p>
      </div>
      <div className="tutorial-actions">
        {step === 0 && (
          <Button onClick={() => void w.importVideo()}>Import video</Button>
        )}
        {step === 1 && (
          <Button disabled={!w.frame || w.busy} onClick={() => w.step(1)}>
            Try next frame
          </Button>
        )}
        {step === 2 && (
          <Button
            disabled={!w.selected.length}
            onClick={() => w.setView("export")}
          >
            Open Export Studio
          </Button>
        )}
        {step > 0 && (
          <Button
            onClick={() => {
              w.setView("extract");
              setStep(step - 1);
            }}
          >
            Back
          </Button>
        )}
        {step < 2 ? (
          <Button
            variant="primary"
            disabled={step === 0 ? !w.activeSource : !w.selected.length}
            onClick={() => setStep(step + 1)}
          >
            Next
          </Button>
        ) : (
          <Button variant="primary" onClick={completeTutorial}>
            Finish tutorial
          </Button>
        )}
        <Button onClick={completeTutorial}>Skip tutorial</Button>
      </div>
    </section>
  );
}
