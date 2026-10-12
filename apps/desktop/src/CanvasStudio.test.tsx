// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import { useJobs } from "./store";
import type { Selection } from "./contracts";

const test = vi.hoisted(() => ({
  selected: [] as Selection[],
  export: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn(),
  import: vi.fn(),
  drop: null as
    null | ((event: { payload: { type: string; paths: string[] } }) => void),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: test.open }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: async (handler: NonNullable<typeof test.drop>) => {
      test.drop = handler;
      return () => {};
    },
  }),
}));
vi.mock("./client", async (original) => {
  const actual = await original<typeof import("./client")>();
  const frame = (ordinal: string) => ({
    ordinal,
    ptsTicks: String(Number(ordinal) * 100),
    timeSeconds: Number(ordinal) / 10,
    durationSeconds: 0.1,
    timestampOrigin: "source",
    keyFrame: ordinal === "0",
  });
  const ref = (source: string, ordinal: string) => ({
    sourceRevisionId: source,
    streamIndex: 0,
    indexGeneration: source,
    presentationOrdinal: ordinal,
    ptsTicks: String(Number(ordinal) * 100),
    timeBaseNumerator: 1,
    timeBaseDenominator: 1000,
    timestampOrigin: "source" as const,
  });
  const view = (source: string, ordinal: string) => ({
    frameRef: ref(source, ordinal),
    timeSeconds: Number(ordinal) / 10,
    width: 640,
    height: 480,
    imagePath: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB",
  });
  return {
    ...actual,
    desktop: true,
    api: {
      projectActivity: async () => ({}),
      markProjectOpened: async () => {},
      projects: async () => [
        { id: "project", name: "Fixture project", createdAt: "" },
      ],
      sources: async () =>
        ["front", "back"].map((id) => ({
          id,
          projectId: "project",
          name: id + ".mkv",
          path: "",
          revisionId: id,
          indexGeneration: id,
          status: "ready",
          frameCount: "20",
          error: null,
          metadata: { selectedStream: 0, durationSeconds: 2, streams: [] },
        })),
      selections: async () => [...test.selected],
      frames: async (_source: string, ordinal: string) => [frame(ordinal)],
      resolve: async (_source: string, time: number) =>
        frame(String(Math.min(19, Math.floor(time * 10)))),
      preview: async (source: string, ordinal: string) => view(source, ordinal),
      thumbnail: async (source: string, ordinal: string) =>
        view(source, ordinal),
      image: (path: string) => path,
      select: async (source: string, ordinal: string, selected: boolean) => {
        test.selected = test.selected.filter(
          (s) =>
            !(
              s.sourceId === source &&
              s.frameRef.presentationOrdinal === ordinal
            ),
        );
        if (selected)
          test.selected.push({
            sourceId: source,
            frameRef: ref(source, ordinal),
            timeSeconds: Number(ordinal) / 10,
          });
      },
      selectAll: async (source: string) => {
        for (let i = 0; i < 20; i++) {
          const ordinal = String(i);
          if (
            !test.selected.some(
              (s) =>
                s.sourceId === source &&
                s.frameRef.presentationOrdinal === ordinal,
            )
          )
            test.selected.push({
              sourceId: source,
              frameRef: ref(source, ordinal),
              timeSeconds: i / 10,
            });
        }
      },
      clearSelections: async () => {
        test.selected = [];
      },
      addSelections: async (_project: string, frames: Selection[]) => {
        for (const f of frames)
          if (
            !test.selected.some(
              (s) =>
                s.sourceId === f.sourceId &&
                s.frameRef.presentationOrdinal ===
                  f.frameRef.presentationOrdinal,
            )
          )
            test.selected.push(f);
      },
      jobs: async () => [],
      onJob: async () => () => {},
      import: test.import,
      export: test.export,
      exportPlans: async () => [],
      recommendations: async () => null,
      cancel: test.cancel,
    },
  };
});
beforeEach(() => {
  test.selected = [];
  test.open.mockReset().mockResolvedValue("/fixture/export");
  test.export.mockReset().mockResolvedValue({
    jobId: "export",
    sequence: 1,
    schemaVersion: 1,
    stage: "exporting",
    status: "queued",
    completedUnits: "0",
    totalUnits: "2",
    resultIds: [],
    error: null,
    messageCode: "queued",
  });
  test.cancel.mockReset().mockResolvedValue(undefined);
  test.import.mockReset().mockResolvedValue({
    jobId: "import",
    sequence: 1,
    schemaVersion: 1,
    status: "queued",
    stage: "indexing",
    completedUnits: "0",
    totalUnits: null,
    resultIds: ["front"],
    error: null,
    messageCode: "queued",
  });
  useJobs.setState({ jobs: {}, order: [] });
});
afterEach(cleanup);

describe("Canvas Studio feature parity", () => {
  it("attaches a dropped video to the active project without opening a file dialog", async () => {
    render(<App />);
    await screen.findByRole("img", { name: "Frame 0 at 00:00:00.000" });
    await act(async () => {
      test.drop!({ payload: { type: "drop", paths: ["/fixture/new.mkv"] } });
    });
    await waitFor(() =>
      expect(test.import).toHaveBeenCalledWith("project", "/fixture/new.mkv"),
    );
    expect(test.open).not.toHaveBeenCalled();
  });
  it("keeps exact navigation, metadata, cross-source selections, and JPEG export connected after moving the controls", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("img", { name: "Frame 0 at 00:00:00.000" });
    await user.click(screen.getByRole("button", { name: "Next frame" }));
    await screen.findByRole("img", { name: "Frame 1 at 00:00:00.100" });
    const timestamp = screen.getByRole("textbox", {
      name: "Seek to timestamp",
    });
    await user.clear(timestamp);
    await user.type(timestamp, "0.7{Enter}");
    await screen.findByRole("img", { name: "Frame 7 at 00:00:00.700" });
    expect(screen.getByRole("region", { name: "Frame details" })).toBeTruthy();
    expect(screen.getByText("PTS ticks")).toBeTruthy();
    expect(screen.getByText("700")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(
      screen.getByRole("img", { name: "Frame 7 at 00:00:00.700" }).style
        .transform,
    ).toContain("scale(1.5)");
    await user.click(
      screen.getByRole("button", { name: "Reset zoom and pan" }),
    );
    expect(
      screen.getByRole("img", { name: "Frame 7 at 00:00:00.700" }).style
        .transform,
    ).toBe("translate(0px,0px) scale(1)");
    await user.click(screen.getByRole("button", { name: "Select frame" }));
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Source video" }),
      "back",
    );
    await screen.findByRole("img", { name: "Frame 0 at 00:00:00.000" });
    await user.click(screen.getByRole("button", { name: "Next frame" }));
    await screen.findByRole("img", { name: "Frame 1 at 00:00:00.100" });
    await user.click(screen.getByRole("button", { name: "Select frame" }));
    await user.click(
      screen.getByRole("button", { name: "Open frame 7 from front.mkv" }),
    );
    await screen.findByRole("img", { name: "Frame 7 at 00:00:00.700" });
    expect(
      screen.getByRole("combobox", { name: "Source video" }),
    ).toHaveProperty("value", "front");
    await user.click(screen.getByRole("button", { name: "Export selected" }));
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Image format" }),
      "jpeg",
    );
    const quality = screen.getByRole("spinbutton", { name: "JPEG quality" });
    await user.clear(quality);
    await user.type(quality, "90");
    await user.click(
      screen.getByRole("button", { name: "Choose folder and export" }),
    );
    await waitFor(() =>
      expect(test.export).toHaveBeenCalledWith("project", "/fixture/export", {
        format: "jpeg",
        jpegQuality: 90,
        colorMode: "sdr",
        colorPolicy: {
          schemaVersion: 2,
          sourcePeakNits: null,
          sourceInterpretation: undefined,
        },
      }),
    );
    for (const format of ["png", "webp"]) {
      const priorJob = Object.values(useJobs.getState().jobs).find(
        (j) => j.stage === "exporting" && j.status === "queued",
      )!;
      act(() =>
        useJobs.getState().setJob({
          ...priorJob,
          status: "succeeded",
          completedUnits: "2",
          sequence: priorJob.sequence + 1,
        }),
      );
      test.export.mockResolvedValue({
        ...priorJob,
        jobId: `export-${format}`,
        status: "queued",
        sequence: 1,
      });
      await user.selectOptions(
        screen.getByRole("combobox", { name: "Image format" }),
        format,
      );
      await user.click(
        screen.getByRole("button", { name: "Choose folder and export" }),
      );
      await waitFor(() =>
        expect(test.export).toHaveBeenLastCalledWith(
          "project",
          "/fixture/export",
          {
            format,
            jpegQuality: 90,
            colorMode: "sdr",
            colorPolicy: {
              schemaVersion: 2,
              sourcePeakNits: null,
              sourceInterpretation: undefined,
            },
          },
        ),
      );
    }
    await user.click(screen.getByRole("button", { name: "Projects" }));
    await user.click(
      await screen.findByRole("button", {
        name: "Open project Fixture project",
      }),
    );
    await screen.findByRole("img", { name: "Frame 0 at 00:00:00.000" });
    await waitFor(() => expect(test.selected).toHaveLength(2));
  });
  it("retains precision-wheel Shift navigation and recoverable cancellation controls", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("img", { name: "Frame 0 at 00:00:00.000" });
    await user.click(screen.getByRole("button", { name: "Expand precision" }));
    await user.click(
      screen.getByRole("button", { name: /Precision frame wheel/ }),
    );
    await user.keyboard("{Shift>}{ArrowRight}{/Shift}");
    await screen.findByRole("img", { name: "Frame 10 at 00:00:01.000" });
    useJobs.getState().setJob({
      schemaVersion: 1,
      jobId: "index",
      sequence: 1,
      status: "running",
      stage: "indexing",
      completedUnits: "12",
      totalUnits: null,
      messageCode: "",
      resultIds: [],
      error: null,
    });
    await user.click(
      await screen.findByRole("button", { name: /1 active job/ }),
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(test.cancel).toHaveBeenCalledWith("index");
    useJobs.getState().setJob({
      schemaVersion: 1,
      jobId: "index",
      sequence: 2,
      status: "cancelling",
      stage: "indexing",
      completedUnits: "12",
      totalUnits: null,
      messageCode: "",
      resultIds: [],
      error: null,
    });
    expect(
      await screen.findByRole("button", { name: "Cancelling…" }),
    ).toHaveProperty("disabled", true);
  });
});

it("adds/removes current and row frames, selects the current source, and clears across sources", async () => {
  const user = userEvent.setup();
  render(<App />);
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Add current frame",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  await user.click(screen.getByRole("button", { name: "Add current frame" }));
  await screen.findByRole("button", { name: "Remove current frame" });
  expect(test.selected).toHaveLength(1);
  await user.click(
    screen.getByRole("button", { name: /Remove frame 0 from front.mkv/ }),
  );
  await screen.findByRole("button", { name: "Add current frame" });
  expect(test.selected).toHaveLength(0);
  await user.click(screen.getByRole("button", { name: "Select all" }));
  await waitFor(() => expect(test.selected).toHaveLength(20));
  expect(test.selected.every((f) => f.sourceId === "front")).toBe(true);
  await user.click(screen.getByRole("button", { name: "Remove all" }));
  await waitFor(() => expect(test.selected).toHaveLength(0));
  await screen.findByRole("button", { name: "Add current frame" });
  expect(screen.queryByRole("button", { name: "Presets" })).toBeNull();
  expect(screen.getByText("Auto-select with a preset")).toBeTruthy();
});
