// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PresetExecutionPanel } from "./components/PresetExecutionPanel";
import { useJobs } from "./store";
import type { JobEvent, PresetResultV1, PresetV1 } from "./contracts";
import type { Workspace } from "./workspace/useWorkspace";
const mock = vi.hoisted(() => ({
  models: vi.fn(),
  subjectChoice: vi.fn(),
  executePreset: vi.fn(),
  presetResult: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock("./client", () => ({
  api: mock,
  message: (e: Error) => e.message,
  timestamp: (n: number) => String(n),
}));
const frame = {
  sourceId: "source",
  timeSeconds: 0,
  frameRef: {
    indexGeneration: "index",
    presentationOrdinal: "9007199254740993",
  },
};
const w = {
  sourceId: "source",
  image: frame,
  openSelection: vi.fn(),
  setError: vi.fn(),
} as unknown as Workspace;
const preset = {
  id: "outfit",
  name: "Outfit",
  version: 1,
  count: 8,
  weights: [20, 15, 25, 30, 10],
  requiredCapabilities: ["person", "pose", "person_mask"],
  automaticAvailable: false,
  unavailableReason: "Unvalidated",
} as PresetV1;
const job: JobEvent = {
  schemaVersion: 1,
  jobId: "run",
  sequence: 0,
  status: "queued",
  stage: "preset_execution",
  completedUnits: "0",
  totalUnits: null,
  messageCode: "QUEUED",
  resultIds: [],
  error: null,
};
beforeEach(() => {
  vi.resetAllMocks();
  useJobs.setState({ jobs: {}, order: [] });
  mock.models.mockResolvedValue([]);
  mock.subjectChoice.mockResolvedValue({
    modelId: "person",
    subject: { personFeatureId: "feature", personIndex: 1 },
    frame,
  });
  mock.executePreset.mockResolvedValue(job);
  mock.cancel.mockResolvedValue(undefined);
});
afterEach(cleanup);
it("requires explicit opt-in and sends the exact anchor and chosen subject", async () => {
  const user = userEvent.setup();
  render(<PresetExecutionPanel w={w} preset={preset} />);
  await waitFor(() => expect(mock.subjectChoice).toHaveBeenCalled());
  const button = screen.getByRole("button", {
    name: "Run from current frame",
  }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  await user.click(screen.getByRole("checkbox"));
  await waitFor(() => expect(button.disabled).toBe(false));
  await user.click(button);
  expect(mock.executePreset).toHaveBeenCalledWith(
    expect.objectContaining({
      experimental: true,
      personModelId: "person",
      personIndex: 1,
      anchorOrdinal: "9007199254740993",
      sourceId: "source",
    }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Cancel execution" }),
  );
  expect(mock.cancel).toHaveBeenCalledWith("run");
});
it("restores an exact recommended frame and explains partial coverage", async () => {
  const user = userEvent.setup();
  render(<PresetExecutionPanel w={w} preset={preset} />);
  await user.click(screen.getByRole("checkbox"));
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Run from current frame",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  await user.click(
    screen.getByRole("button", { name: "Run from current frame" }),
  );
  mock.presetResult.mockResolvedValue({
    track: { coverageComplete: false, stopReason: "ambiguous_subjects" },
    frames: [
      { quality: { frame }, score: 70, reasons: ["Sharp target region"] },
    ],
  } as unknown as PresetResultV1);
  act(() =>
    useJobs.getState().setJob({ ...job, sequence: 1, status: "succeeded" }),
  );
  expect(
    await screen.findByText(/Partial coverage · ambiguous subjects/),
  ).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: /Frame 9007199254740993/ }),
  );
  expect(w.openSelection).toHaveBeenCalledWith(frame);
});
it("keeps a missing automatic capability unavailable", async () => {
  render(
    <PresetExecutionPanel
      w={w}
      preset={{ ...preset, requiredCapabilities: ["hair_mask"] }}
    />,
  );
  expect(
    screen.getByText(/Required automatic model capability unavailable/),
  ).toBeTruthy();
  expect(
    (
      screen.getByRole("button", {
        name: "Run from current frame",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
});
