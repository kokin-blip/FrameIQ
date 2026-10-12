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
import type { SourceSummary } from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";

type Cover = {
  url?: string;
  sourceCount: number;
  error?: string;
  loading?: boolean;
  sourceId?: string;
  sources?: SourceSummary[];
};
function Rename({
  name,
  label,
  save,
  onError,
}: {
  name: string;
  label: string;
  save: (name: string) => Promise<void>;
  onError: (message: string) => void;
}) {
  const [editing, setEditing] = useState(false),
    [value, setValue] = useState(name),
    [busy, setBusy] = useState(false);
  return editing ? (
    <form
      className="rename-form"
      onSubmit={(e) => {
        e.preventDefault();
        setBusy(true);
        void save(value.trim())
          .then(() => setEditing(false))
          .catch((e) => onError(message(e)))
          .finally(() => setBusy(false));
      }}
    >
      <label>
        {label}
        <input
          autoFocus
          value={value}
          maxLength={120}
          onChange={(e) => setValue(e.target.value)}
        />
      </label>
      <Button disabled={busy || !value.trim()}>Save</Button>
      <Button type="button" disabled={busy} onClick={() => setEditing(false)}>
        Cancel
      </Button>
    </form>
  ) : (
    <Button
      aria-label={`${label}: ${name}`}
      onClick={() => {
        setValue(name);
        setEditing(true);
      }}
    >
      {label}
    </Button>
  );
}
export function ProjectLibrary({ w }: { w: Workspace }) {
  const [query, setQuery] = useState(""),
    [sort, setSort] = useState("recent"),
    [activity, setActivity] = useState<Record<string, string>>({});
  useEffect(() => {
    void api
      .projectActivity()
      .then(setActivity)
      .catch((e) => w.setError(message(e)));
  }, [w.projects]);
  const projects = w.projects
    .filter((p) =>
      p.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()),
    )
    .sort((a, b) =>
      sort === "name"
        ? a.name.localeCompare(b.name)
        : (activity[b.id] ?? b.createdAt).localeCompare(
            activity[a.id] ?? a.createdAt,
          ),
    );
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
            sources,
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
      <div className="library-tools">
        <label>
          Search projects
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <label>
          Sort by
          <select value={sort} onChange={(e) => setSort(e.target.value)}>
            <option value="recent">Last opened</option>
            <option value="name">Name</option>
          </select>
        </label>
      </div>
      {w.projects.length ? (
        <div className="project-grid">
          {projects.map((p) => {
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
                <div className="project-management">
                  <p>
                    {activity[p.id]
                      ? `Last opened ${new Date(activity[p.id]).toLocaleString()}`
                      : "Last opened: not recorded"}
                  </p>
                  <Rename
                    name={p.name}
                    label="Rename project"
                    save={(name) => w.renameProject(p.id, name)}
                    onError={w.setError}
                  />
                  <details>
                    <summary>Sources ({cover?.sourceCount ?? 0})</summary>
                    {cover?.sources?.map((source) => (
                      <div className="source-alias" key={source.id}>
                        <strong>{source.name}</strong>
                        <small>File: {source.path.split(/[\\/]/).pop()}</small>
                        <Rename
                          name={source.name}
                          label="Rename source"
                          save={(name) => w.renameSource(source.id, name)}
                          onError={w.setError}
                        />
                      </div>
                    ))}
                  </details>
                </div>
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
          {!projects.length && <p>No matching projects.</p>}
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
