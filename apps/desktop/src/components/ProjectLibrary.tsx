import { useEffect, useState } from "react";
import { Button, EmptyState } from "@frameiq/ui";
import {
  ArrowUpRight,
  Film,
  FolderOpen,
  Plus,
  AlertCircle,
} from "lucide-react";
import { api, message } from "../client";
import type { Workspace } from "../workspace/useWorkspace";

type Cover = {
  url?: string;
  sourceCount: number;
  error?: string;
  loading?: boolean;
  sourceId?: string;
};
export function ProjectLibrary({ w }: { w: Workspace }) {
  const [covers, setCovers] = useState<Record<string, Cover>>({});
  useEffect(() => {
    let alive = true;
    void (async () => {
      // One thumbnail request at a time; native decoding stays bounded.
      for (const project of w.projects) {
        try {
          const sources = await api.sources(project.id);
          const source = sources.find(
            (s) => s.status === "ready" && BigInt(s.frameCount) > 0n,
          );
          let cover: Cover = {
            sourceCount: sources.length,
            sourceId: source?.id,
          };
          if (source) {
            try {
              const image = await api.thumbnail(source.id, "0");
              cover.url = api.image(image.imagePath);
            } catch (e) {
              cover.error = message(e);
            }
          } else if (sources.some((s) => s.error))
            cover.error = sources.find((s) => s.error)!.error!.message;
          else if (sources.length) cover.loading = true;
          if (!alive) return;
          setCovers((prev) => ({ ...prev, [project.id]: cover }));
        } catch (e) {
          if (!alive) return;
          setCovers((prev) => ({
            ...prev,
            [project.id]: { sourceCount: 0, error: message(e) },
          }));
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, [w.projects]);
  return (
    <section className="library-page page">
      <div className="page-title">
        <div>
          <h1>Project library</h1>
          <p>Manage videos, selected frames, and exports.</p>
        </div>
        <Button onClick={() => void w.importVideo()}>
          <Plus size={16} />
          Import video
        </Button>
      </div>
      <form
        className="create-project"
        onSubmit={(e) => {
          e.preventDefault();
          void w.createProject();
        }}
      >
        <label htmlFor="project-name">Create a project</label>
        <div>
          <input
            id="project-name"
            aria-label="New project name"
            placeholder="Project name"
            value={w.newProject}
            onChange={(e) => w.setNewProject(e.target.value)}
            maxLength={120}
          />
          <Button variant="primary" disabled={!w.newProject.trim()}>
            <Plus size={16} />
            Create project
          </Button>
        </div>
      </form>
      {w.projects.length ? (
        <div className="project-grid">
          {w.projects.map((p) => {
            const cover = covers[p.id];
            return (
              <article className="project-card" key={p.id}>
                <button
                  className="project-open"
                  onClick={() => w.openProject(p)}
                  aria-label={`Open project ${p.name}`}
                >
                  <div className="project-cover">
                    {cover?.url ? (
                      <img src={cover.url} alt="" />
                    ) : cover?.error ? (
                      <AlertCircle size={28} />
                    ) : (
                      <Film size={32} />
                    )}
                    <span>
                      {cover?.error
                        ? "Source unavailable"
                        : cover?.loading
                          ? "Indexing source"
                          : cover?.sourceCount
                            ? "Video project"
                            : "Empty project"}
                    </span>
                  </div>
                  <div className="project-card-title">
                    <div>
                      <h2>{p.name}</h2>
                      <p>
                        {cover
                          ? `${cover.sourceCount} ${cover.sourceCount === 1 ? "source" : "sources"}`
                          : "Loading sources…"}
                      </p>
                    </div>
                    <ArrowUpRight size={18} />
                  </div>
                </button>
                {cover?.error && (
                  <div className="project-recovery">
                    <p>{cover.error}</p>
                    {cover.sourceId && (
                      <Button
                        onClick={() => void w.locateSource(cover.sourceId!)}
                      >
                        Locate file
                      </Button>
                    )}
                    <Button onClick={() => void w.importVideo(p)}>
                      Import replacement
                    </Button>
                  </div>
                )}
              </article>
            );
          })}
        </div>
      ) : (
        <EmptyState
          title="No projects"
          action={
            <Button variant="primary" onClick={() => void w.importVideo()}>
              <FolderOpen size={16} />
              Choose a video
            </Button>
          }
        >
          Attach a video to create a project.
        </EmptyState>
      )}
    </section>
  );
}
