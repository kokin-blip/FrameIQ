import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, desktop, message, timestamp } from "../client";
import { useJobs } from "../store";
import type {
  Project,
  SourceSummary,
  IndexedFrame,
  FrameView,
  Selection,
  ExportFormat,
  ColorMode,
} from "../contracts";
export type View = "library" | "extract" | "export" | "settings";
export function useWorkspace() {
  const [view, setView] = useState<View>("extract"),
    [projects, setProjects] = useState<Project[]>([]),
    [project, setProject] = useState<Project | null>(null),
    [sources, setSources] = useState<SourceSummary[]>([]),
    [sourceId, setSourceId] = useState<string | null>(null),
    [frame, setFrame] = useState<IndexedFrame | null>(null),
    [image, setImage] = useState<FrameView | null>(null),
    [selected, setSelected] = useState<Selection[]>([]),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [timeInput, setTimeInput] = useState("00:00:00.000"),
    [playing, setPlaying] = useState(false),
    [format, setFormat] = useState<ExportFormat>("png"),
    [quality, setQuality] = useState(95),
    [colorMode, setColorMode] = useState<ColorMode>("sdr"),
    [sourceColor, setSourceColor] = useState(""),
    [zoom, setZoom] = useState(1),
    [pan, setPan] = useState({ x: 0, y: 0 }),
    [thumbs, setThumbs] = useState<{ frame: IndexedFrame; url: string }[]>([]),
    [newProject, setNewProject] = useState("");
  const [dropActive, setDropActive] = useState(false);
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob);
  const activeSource = sources.find((s) => s.id === sourceId) ?? null;
  const request = useRef(0),
    desired = useRef("0"),
    lastFrame = useRef<IndexedFrame | null>(null);
  lastFrame.current = frame;
  const currentProject = useRef<string | null>(null);
  currentProject.current = project?.id ?? null;
  const pendingFocus = useRef<string | null>(null);
  const reload = useCallback(async (p: Project) => {
    const [s, sel] = await Promise.all([
      api.sources(p.id),
      api.selections(p.id),
    ]);
    if (currentProject.current !== p.id) return;
    setSources(s);
    setSelected(sel);
  }, []);
  useEffect(() => {
    let alive = true;
    api
      .projects()
      .then((p) => {
        if (alive) {
          setProjects(p);
          setProject(p[0] ?? null);
        }
      })
      .catch((e) => setError(message(e)));
    api
      .jobs()
      .then((j) => j.reverse().forEach(setJob))
      .catch((e) => setError(message(e)));
    let dispose = () => {};
    api
      .onJob((e) => {
        setJob(e);
        if (e.status === "failed" && e.error) setError(e.error.message);
      })
      .then((fn) => {
        if (alive) dispose = fn;
        else fn();
      });
    return () => {
      alive = false;
      dispose();
    };
  }, [setJob]);
  useEffect(() => {
    if (!project) return;
    reload(project).catch((e) => setError(message(e)));
    const id = window.setInterval(
      () => reload(project).catch((e) => setError(message(e))),
      1000,
    );
    return () => clearInterval(id);
  }, [project, reload]);
  useEffect(() => {
    if (!sourceId && sources.length) setSourceId(sources[0].id);
  }, [sources, sourceId]);
  const show = useCallback(async (source: string, ordinal: string) => {
    desired.current = ordinal;
    const id = ++request.current;
    setBusy(true);
    try {
      const f = (await api.frames(source, ordinal, 1))[0];
      if (!f) throw { message: "This frame is still being indexed." };
      if (id !== request.current) return;
      setFrame(f);
      setTimeInput(timestamp(f.timeSeconds));
      const result = await api.preview(source, ordinal);
      if (id === request.current) {
        setImage(result);
        setError("");
      }
    } catch (e) {
      if (
        id === request.current &&
        !(typeof e === "object" && e && "code" in e && e.code === "CANCELLED")
      )
        setError(message(e));
    } finally {
      if (id === request.current) setBusy(false);
    }
  }, []);
  useEffect(() => {
    ++request.current;
    setImage(null);
    setFrame(null);
    setPlaying(false);
    setZoom(1);
    setPan({ x: 0, y: 0 });
    setThumbs([]);
    desired.current = pendingFocus.current ?? "0";
    if (sourceId && BigInt(activeSource?.frameCount ?? "0") > 0n)
      void show(sourceId, desired.current);
    pendingFocus.current = null;
  }, [sourceId, show]);
  useEffect(() => {
    if (sourceId && !frame && BigInt(activeSource?.frameCount ?? "0") > 0n)
      void show(sourceId, desired.current);
  }, [sourceId, activeSource?.frameCount, frame, show]);
  useEffect(() => {
    if (!activeSource || activeSource.status !== "ready") return;
    let alive = true;
    const s = activeSource;
    const duration = s.metadata?.durationSeconds ?? 0;
    (async () => {
      const list: { frame: IndexedFrame; url: string }[] = [];
      const seen = new Set<string>();
      for (let i = 0; i < 12 && alive; i++) {
        const f = await api.resolve(s.id, (duration * i) / 12);
        if (seen.has(f.ordinal)) continue;
        seen.add(f.ordinal);
        const image = await api.thumbnail(s.id, f.ordinal);
        if (!alive) return;
        list.push({ frame: f, url: api.image(image.imagePath) });
        setThumbs([...list]);
      }
    })().catch((e) => {
      if (alive) setError(message(e));
    });
    return () => {
      alive = false;
    };
  }, [activeSource?.id, activeSource?.status]);
  const step = useCallback(
    (delta: number) => {
      if (!activeSource) return;
      const max = BigInt(activeSource.frameCount) - 1n;
      if (max < 0n) return;
      let n = BigInt(desired.current) + BigInt(delta);
      if (n < 0n) n = 0n;
      if (n > max) n = max;
      setPlaying(false);
      void show(activeSource.id, n.toString());
    },
    [activeSource, show],
  );
  const seek = useCallback(
    async (seconds: number) => {
      if (!activeSource) return;
      try {
        const f = await api.resolve(activeSource.id, seconds);
        await show(activeSource.id, f.ordinal);
      } catch (e) {
        setError(message(e));
      }
    },
    [activeSource, show],
  );
  const toggleSelection = useCallback(async () => {
    if (!project || !activeSource || !frame) return;
    const exists = selected.some(
      (s) =>
        s.sourceId === activeSource.id &&
        s.frameRef.presentationOrdinal === frame.ordinal,
    );
    try {
      await api.select(activeSource.id, frame.ordinal, !exists);
      await reload(project);
    } catch (e) {
      setError(message(e));
    }
  }, [project, activeSource, frame, selected, reload]);
  useEffect(() => {
    function key(e: KeyboardEvent) {
      if (
        e.target instanceof HTMLElement &&
        (e.target.matches("input,textarea,select") ||
          e.target.isContentEditable)
      )
        return;
      if (view !== "extract") return;
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        step((e.key === "ArrowLeft" ? -1 : 1) * (e.shiftKey ? 10 : 1));
      }
      if (e.key === " ") {
        if (
          !frame ||
          (e.target instanceof HTMLElement &&
            e.target.closest("button,a,[role=button]"))
        )
          return;
        e.preventDefault();
        setPlaying((p) => !p);
      }
      if (e.key.toLowerCase() === "s") {
        e.preventDefault();
        void toggleSelection();
      }
    }
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [step, view, toggleSelection, frame]);
  useEffect(() => {
    if (!playing || !activeSource) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    const start = performance.now(),
      origin = lastFrame.current?.timeSeconds ?? 0;
    async function tick() {
      if (!alive) return;
      const time = origin + (performance.now() - start) / 1000;
      if (time >= (activeSource?.metadata?.durationSeconds ?? Infinity)) {
        setPlaying(false);
        return;
      }
      await seek(time);
      if (alive) timer = setTimeout(tick, 80);
    }
    void tick();
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [playing, activeSource?.id, seek]);
  async function importVideo(targetProject?: Project, droppedPath?: string) {
    try {
      if (!desktop)
        throw { message: "Open the desktop app to import a local video." };
      // Native extension filters can reject containers with an unregistered macOS UTI.
      // FFmpeg validates the selected file instead.
      const path =
        droppedPath ?? (await open({ multiple: false, directory: false }));
      if (typeof path !== "string") return;
      let p = targetProject ?? project;
      if (targetProject) {
        setProject(targetProject);
        setSources([]);
        setSourceId(null);
      }
      if (!p) {
        p = await api.createProject("My first project");
        setProjects((prev) => [p!, ...prev]);
        setProject(p);
      }
      const job = await api.import(p.id, path);
      setJob(job);
      await reload(p);
      setSourceId(job.resultIds[0] ?? null);
      setView("extract");
      setError("");
    } catch (e) {
      setError(message(e));
    }
  }
  const dropHandler = useRef<(paths: string[]) => void>(() => {});
  dropHandler.current = (paths) => {
    if (paths.length !== 1) {
      setError("Attach one video at a time.");
      return;
    }
    void importVideo(undefined, paths[0]);
  };
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    let dispose = () => {};
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (!alive) return;
        const payload = event.payload;
        setDropActive(payload.type === "enter" || payload.type === "over");
        if (payload.type === "drop") dropHandler.current(payload.paths);
      })
      .then((fn) => {
        if (alive) dispose = fn;
        else fn();
      })
      .catch((e) => {
        if (alive) setError(message(e));
      });
    return () => {
      alive = false;
      dispose();
    };
  }, []);
  async function createProject() {
    try {
      const p = await api.createProject(newProject);
      setProjects((prev) => [...prev, p]);
      setProject(p);
      setSourceId(null);
      setSources([]);
      setSelected([]);
      setNewProject("");
      setView("extract");
    } catch (e) {
      setError(message(e));
    }
  }
  async function locateSource(id: string) {
    try {
      const path = await open({ multiple: false, directory: false });
      if (typeof path !== "string") return;
      setJob(await api.relocate(id, path));
      setError("");
    } catch (e) {
      setError(message(e));
    }
  }
  async function exportFrames() {
    try {
      const path = await open({ directory: true, multiple: false });
      if (typeof path !== "string" || !project) return;
      const job = await api.export(project.id, path, {
        format,
        jpegQuality: quality,
        colorMode,
        colorPolicy: {
          schemaVersion: 2,
          sourcePeakNits: null,
          sourceInterpretation: sourceColor || undefined,
        },
      });
      setJob(job);
      setError("");
    } catch (e) {
      setError(message(e));
    }
  }
  const isSelected =
    !!frame &&
    selected.some(
      (s) =>
        s.sourceId === sourceId &&
        s.frameRef.presentationOrdinal === frame.ordinal,
    );
  const activeJobs = Object.values(jobs).filter((j) =>
    ["queued", "running", "cancelling"].includes(j.status),
  );
  const currentStream = activeSource?.metadata?.streams.find(
    (s) => s.index === activeSource.metadata?.selectedStream,
  );

  function openProject(p: Project) {
    setSources([]);
    setSelected([]);
    setProject(p);
    setSourceId(null);
    setView("extract");
  }
  function openSelection(s: Selection) {
    setPlaying(false);
    setView("extract");
    if (s.sourceId !== sourceId) {
      pendingFocus.current = s.frameRef.presentationOrdinal;
      setSourceId(s.sourceId);
    } else void show(s.sourceId, s.frameRef.presentationOrdinal);
  }
  function chooseSource(id: string) {
    setPlaying(false);
    setSourceId(id);
  }
  return {
    view,
    setView,
    projects,
    project,
    sources,
    sourceId,
    frame,
    image,
    selected,
    error,
    setError,
    busy,
    timeInput,
    setTimeInput,
    playing,
    setPlaying,
    format,
    setFormat,
    sourceColor,
    setSourceColor,
    colorMode,
    setColorMode,
    quality,
    setQuality,
    zoom,
    setZoom,
    pan,
    setPan,
    thumbs,
    newProject,
    setNewProject,
    activeSource,
    step,
    seek,
    show,
    toggleSelection,
    importVideo,
    createProject,
    exportFrames,
    locateSource,
    isSelected,
    activeJobs,
    currentStream,
    openProject,
    openSelection,
    chooseSource,
    dropActive,
  };
}
export type Workspace = ReturnType<typeof useWorkspace>;
