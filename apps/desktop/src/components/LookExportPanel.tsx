import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@frameiq/ui";
import { fashionApi } from "../fashionClient";
import { message } from "../client";
import { useJobs } from "../store";
import { useFashionTab } from "../fashionPreferences";
import type { SavedLookV1, FashionExportV1 } from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";
export function LookExportPanel({ w }: { w: Workspace }) {
  const [looks, setLooks] = useState<SavedLookV1[]>([]),
    [plans, setPlans] = useState<FashionExportV1[]>([]),
    [id, setId] = useState(""),
    [format, setFormat] = useState("html"),
    [busy, setBusy] = useState(false);
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob),
    project = w.project?.id;
  useEffect(() => {
    let live = true;
    if (!project) return;
    void Promise.all([fashionApi.looks(project), fashionApi.exports(project)])
      .then(([l, p]) => {
        if (live) {
          setLooks(l);
          setPlans(p);
        }
      })
      .catch((e) => w.setError(message(e)));
    return () => {
      live = false;
    };
  }, [project, jobs]);
  async function exportLook() {
    if (!project) return;
    setBusy(true);
    try {
      const destination = await open({ directory: true, multiple: false });
      if (typeof destination === "string")
        setJob(await fashionApi.exportLook(project, id, destination, format));
    } catch (e) {
      w.setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="export-summary look-export-panel">
      <div>
        <h2>Fashion lookbooks</h2>
        <p>
          Export saved looks with item crops, alternatives, unresolved items and
          source evidence. Product imagery follows its recorded copying rights.
        </p>
        <label>
          Saved look
          <select value={id} onChange={(e) => setId(e.target.value)}>
            <option value="">Choose a look</option>
            {looks.map((l) => (
              <option key={l.id} value={l.id}>
                {l.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Lookbook format
          <select value={format} onChange={(e) => setFormat(e.target.value)}>
            <option value="html">Self-contained HTML</option>
            <option value="json">JSON and assets</option>
            <option value="reference">Reference pack</option>
          </select>
        </label>
        <div className="fashion-actions">
          <Button disabled={busy || !id} onClick={() => void exportLook()}>
            Export lookbook
          </Button>
          <Button
            onClick={() => {
              useFashionTab.getState().setTab("looks");
              w.setView("fashion");
            }}
          >
            Manage saved looks
          </Button>
        </div>
        {plans.map((p) => (
          <p key={p.id}>
            {p.look.name} · {p.status} · {p.completed.length}/{p.files.length}{" "}
            <Button
              disabled={busy}
              onClick={() => {
                setBusy(true);
                void fashionApi
                  .resumeExport(p.id)
                  .then(setJob)
                  .catch((e) => w.setError(message(e)))
                  .finally(() => setBusy(false));
              }}
            >
              Verify / resume
            </Button>
          </p>
        ))}
        {!looks.length && (
          <p>Save a look in Fashion inspection to export it here.</p>
        )}
      </div>
    </section>
  );
}
