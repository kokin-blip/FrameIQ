import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { api, desktop, message, timestamp } from "../client";
import { useJobs, usePreferences } from "../store";
import type {
  Project,
  SourceSummary,
  IndexedFrame,
  FrameView,
  Selection,
  ExportFormat,
  ColorMode,
} from "../contracts";
export type View = "library" | "extract" | "export" | "settings" | "setup";
export function useWorkspace() {
  const [view, setView] = useState<View>(() =>
      desktop && !usePreferences.getState().setupCompleted
        ? "setup"
        : "extract",
    ),
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
    [zoomMode, setZoomMode] = useState<"fit" | "pixels">("fit"),
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
  const reloadGeneration = useRef(0);
  const reload = useCallback(async (p: Project) => {
    const generation = reloadGeneration.current;
    const [s, sel] = await Promise.all([
      api.sources(p.id),
      api.selections(p.id),
    ]);
    if (
      currentProject.current !== p.id ||
      generation !== reloadGeneration.current
    )
      return;
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
    if (project && view === "extract")
      void api.markProjectOpened(project.id).catch((e) => setError(message(e)));
  }, [project?.id, view]);
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
    setZoomMode("fit");
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
  const [selectionBusy, setSelectionBusy] = useState(false);
  const selectionLock = useRef(false);
  const mutateSelections = useCallback(
    async (operation: () => Promise<void>) => {
      if (!project || selectionLock.current)
        throw new Error("Wait for the current selection operation.");
      selectionLock.current = true;
      setSelectionBusy(true);
      try {
        await operation();
        await reload(project);
      } catch (e) {
        setError(message(e));
        throw e;
      } finally {
        selectionLock.current = false;
        setSelectionBusy(false);
      }
    },
    [project, reload],
  );
  const addSelections = useCallback(
    async (frames: Selection[]) => {
      if (!project) return;
      await mutateSelections(() => api.addSelections(project.id, frames));
    },
    [project, mutateSelections],
  );
  const removeSelection = useCallback(
    async (s: Selection) => {
      await mutateSelections(() =>
        api.select(s.sourceId, s.frameRef.presentationOrdinal, false),
      );
    },
    [mutateSelections],
  );
  const selectAll = useCallback(async () => {
    if (activeSource)
      await mutateSelections(() => api.selectAll(activeSource.id));
  }, [activeSource, mutateSelections]);
  const clearSelections = useCallback(async () => {
    if (project) await mutateSelections(() => api.clearSelections(project.id));
  }, [project, mutateSelections]);
  async function prepareSwitch() {
    if (selectionLock.current)
      throw new Error("Wait for the current selection operation.");
    if (!project || !usePreferences.getState().clearSelectionsOnSwitch) return;
    if (
      Object.values(useJobs.getState().jobs).some(
        (j) =>
          j.stage === "preset_execution" &&
          ["queued", "running", "cancelling"].includes(j.status),
      )
    )
      throw new Error(
        "Cancel automatic frame selection before opening another file or project.",
      );
    ++reloadGeneration.current;
    await mutateSelections(async () => {
      await api.clearSelections(project.id);
      ++reloadGeneration.current;
      setSelected([]);
    });
  }
  const toggleSelection = useCallback(async () => {
    if (!project || !activeSource || !frame) return;
    const exists = selected.some(
      (s) =>
        s.sourceId === activeSource.id &&
        s.frameRef.presentationOrdinal === frame.ordinal,
    );
    try {
      await mutateSelections(() =>
        api.select(activeSource.id, frame.ordinal, !exists),
      );
    } catch (e) {
      setError(message(e));
    }
  }, [project, activeSource, frame, selected, mutateSelections]);
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
      if (targetProject && targetProject.id !== project?.id) {
        await prepareSwitch();
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
      if (p.id === project?.id && job.resultIds[0] !== sourceId)
        await prepareSwitch();
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
      await prepareSwitch();
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

  async function renameProject(id: string, name: string) {
    await api.renameProject(id, name);
    const list = await api.projects();
    setProjects(list);
    if (project?.id === id)
      setProject(list.find((p) => p.id === id) ?? project);
  }
  async function renameSource(id: string, name: string) {
    await api.renameSource(id, name);
    if (project) await reload(project);
    setProjects(await api.projects());
  }
  async function openProject(p: Project) {
    try {
      if (p.id !== project?.id) await prepareSwitch();
      ++reloadGeneration.current;
      setSources([]);
      setSelected([]);
      setProject(p);
      setSourceId(null);
      setView("extract");
    } catch (e) {
      setError(message(e));
    }
  }
  function openSelection(s: Selection) {
    setPlaying(false);
    setView("extract");
    if (s.sourceId !== sourceId) {
      pendingFocus.current = s.frameRef.presentationOrdinal;
      setSourceId(s.sourceId);
    } else void show(s.sourceId, s.frameRef.presentationOrdinal);
  }
  async function chooseSource(id: string) {
    if (id === sourceId) return;
    try {
      await prepareSwitch();
      setPlaying(false);
      setSourceId(id);
    } catch (e) {
      setError(message(e));
    }
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
    zoomMode,
    setZoomMode,
    renameProject,
    renameSource,
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
    selectionBusy,
    refreshSelections: async () => {
      if (project) await reload(project);
    },
    addSelections,
    removeSelection,
    selectAll,
    clearSelections,
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
    openModelSetup: () => setView("setup"),
    dropActive,
  };
}
export type Workspace = ReturnType<typeof useWorkspace>;
