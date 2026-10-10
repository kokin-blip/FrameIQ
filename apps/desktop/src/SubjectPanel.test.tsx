// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { SubjectPanel } from "./components/SubjectPanel";
import { useJobs } from "./store";
import type {
  JobEvent,
  ModelInstallationV1,
  PersonFrameV1,
  SubjectChoiceV1,
} from "./contracts";
import type { Workspace } from "./workspace/useWorkspace";
const mock = vi.hoisted(() => ({
  models: vi.fn(),
  personFeature: vi.fn(),
  subjectChoice: vi.fn(),
  chooseSubject: vi.fn(),
  clearSubject: vi.fn(),
  poseFeature: vi.fn(),
  detectPersons: vi.fn(),
  estimatePose: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock("./client", () => ({
  api: { ...mock, image: (path: string) => path },
  message: (e: unknown) => String((e as { message?: string }).message ?? e),
}));
const frameRef = (ordinal = "0") => ({
  sourceRevisionId: "revision",
  streamIndex: 0,
  indexGeneration: "index",
  presentationOrdinal: ordinal,
  ptsTicks: ordinal,
  timeBaseNumerator: 1,
  timeBaseDenominator: 10,
  timestampOrigin: "source" as const,
});
const people = (ordinal = "0"): PersonFrameV1 => ({
  id: `people-${ordinal}`,
  modelId: "detector",
  colorPolicy: "reference_preview_sdr",
  frame: {
    sourceId: "source",
    frameRef: frameRef(ordinal),
    timeSeconds: Number(ordinal) / 10,
  },
  result: {
    width: 640,
    height: 480,
    algorithm: "person-v1",
    modelHash: "hash",
    persons: [0, 1].map((i) => ({
      detectorBox: [i * 200, 10, i * 200 + 100, 200],
      landmarks: [
        [0, 0],
        [0, 0],
        [0, 0],
        [0, 0],
      ],
      score: 0.9,
    })),
  },
});
const choice = (index = 0, ordinal = "0"): SubjectChoiceV1 => ({
  modelId: "detector",
  subject: { personFeatureId: `people-${ordinal}`, personIndex: index },
  frame: people(ordinal).frame,
});
const workspace = (ordinal = "0") =>
  ({
    sourceId: "source",
    image: {
      width: 640,
      height: 480,
      imagePath: "/frame.png",
      frameRef: frameRef(ordinal),
      timeSeconds: 0,
    },
    activeSource: { indexGeneration: "index" },
    setPlaying: vi.fn(),
    setError: vi.fn(),
    busy: false,
  }) as unknown as Workspace;
const job: JobEvent = {
  schemaVersion: 1,
  jobId: "detect",
  sequence: 0,
  status: "queued",
  stage: "person_detection",
  completedUnits: "0",
  totalUnits: null,
  messageCode: "QUEUED",
  resultIds: [],
  error: null,
};
beforeEach(() => {
  vi.resetAllMocks();
  useJobs.setState({ jobs: {}, order: [] });
  mock.models.mockResolvedValue(
    ["person_detector", "pose"].map((task, i) => ({
      id: i ? "pose" : "detector",
      status: "ready",
      manifest: { task, name: i ? "Pose model" : "Person model" },
    })) as ModelInstallationV1[],
  );
  mock.personFeature.mockResolvedValue(people());
  mock.subjectChoice.mockResolvedValue(null);
  mock.poseFeature.mockResolvedValue(null);
  mock.chooseSubject.mockImplementation(async (_id, index) => {
    const c = choice(index);
    mock.subjectChoice.mockResolvedValue(c);
    return c;
  });
  mock.clearSubject.mockImplementation(async () => {
    mock.subjectChoice.mockResolvedValue(null);
  });
  mock.detectPersons.mockResolvedValue(job);
  mock.cancel.mockResolvedValue(undefined);
});
afterEach(cleanup);
it("keeps manual workflows available when no compatible model is installed", async () => {
  mock.models.mockResolvedValue([]);
  render(<SubjectPanel w={workspace()} />);
  expect(
    await screen.findByText(
      "Install a compatible person detector in Settings.",
    ),
  ).toBeTruthy();
  expect(
    (screen.getByRole("button", { name: "Detect people" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(mock.detectPersons).not.toHaveBeenCalled();
});
it("chooses and clears the explicit person using keyboard-accessible controls", async () => {
  const user = userEvent.setup();
  render(<SubjectPanel w={workspace()} />);
  const button = await screen.findByRole("button", { name: "Person 2" });
  button.focus();
  await user.keyboard("{Enter}");
  expect(mock.chooseSubject).toHaveBeenCalledWith("people-0", 1);
  expect(
    (
      await screen.findByRole("button", { name: "Person 2 · Selected" })
    ).getAttribute("aria-pressed"),
  ).toBe("true");
  await user.click(screen.getByRole("button", { name: "Clear subject" }));
  await waitFor(() =>
    expect(mock.clearSubject).toHaveBeenCalledWith("source", "0"),
  );
  await screen.findByRole("button", { name: "Person 2" });
});
it("does not show delayed detection results on a different indexed frame", async () => {
  let resolve!: (p: PersonFrameV1) => void;
  mock.personFeature.mockImplementation((_source, n) =>
    n === "0"
      ? new Promise<PersonFrameV1>((r) => {
          resolve = r;
        })
      : Promise.resolve(null),
  );
  const view = render(<SubjectPanel w={workspace()} />);
  await waitFor(() => expect(resolve).toBeDefined());
  view.rerender(<SubjectPanel w={workspace("1")} />);
  await waitFor(() =>
    expect(mock.personFeature).toHaveBeenCalledWith("source", "1", "detector"),
  );
  await act(async () => {
    resolve(people());
  });
  expect(screen.queryByRole("button", { name: "Person 1" })).toBeNull();
});
it("exposes cancellation and retains actionable worker errors", async () => {
  const user = userEvent.setup();
  render(<SubjectPanel w={workspace()} />);
  await screen.findByRole("button", { name: "Person 1" });
  await user.click(screen.getByRole("button", { name: "Detect people" }));
  await user.click(await screen.findByRole("button", { name: "Cancel" }));
  expect(mock.cancel).toHaveBeenCalledWith("detect");
  act(() =>
    useJobs.getState().setJob({ ...job, status: "cancelling", sequence: 1 }),
  );
  expect(
    (screen.getByRole("button", { name: "Cancel" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  act(() =>
    useJobs.getState().setJob({
      ...job,
      status: "failed",
      sequence: 2,
      error: { code: "MEMORY_LIMIT", message: "Free memory and retry." },
    }),
  );
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "Free memory and retry.",
  );
  expect(
    (screen.getByRole("button", { name: "Detect people" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
});
it("restores a saved subject and shows experimental landmarks without enabling visibility scoring", async () => {
  mock.subjectChoice.mockResolvedValue(choice());
  mock.poseFeature.mockResolvedValue({
    id: "pose-result",
    modelId: "pose",
    personFeatureId: "people-0",
    personIndex: 0,
    frame: people().frame,
    colorPolicy: "sdr",
    result: {
      width: 640,
      height: 480,
      pose: {
        landmarks: Array.from({ length: 39 }, () => ({
          x: 100,
          y: 100,
          z: 0,
          visibility: 0.9,
          presence: 0.9,
        })),
      },
      maskPath: "/mask.png",
    },
  });
  render(<SubjectPanel w={workspace()} />);
  await screen.findByRole("button", { name: "Person 1 · Selected" });
  expect(
    await screen.findByText("39 landmarks · person mask available"),
  ).toBeTruthy();
  expect(screen.getByLabelText("Show person mask")).toBeTruthy();
  expect(
    screen.getByText(
      "Pose inspection is experimental. Automatic visibility scoring is unavailable.",
    ),
  ).toBeTruthy();
});

it("restores the saved detector when several compatible installations exist", async () => {
  mock.models.mockResolvedValue([
    {
      id: "other",
      status: "ready",
      manifest: { task: "person_detector", name: "Other detector" },
    },
    {
      id: "detector",
      status: "ready",
      manifest: { task: "person_detector", name: "Saved detector" },
    },
  ]);
  mock.subjectChoice.mockImplementation(async () => {
    await new Promise((r) => setTimeout(r, 20));
    return choice();
  });
  mock.personFeature.mockImplementation(async (_source, _n, model) =>
    model === "detector" ? people() : null,
  );
  render(<SubjectPanel w={workspace()} />);
  await screen.findByRole("button", { name: "Person 1 · Selected" });
  expect(
    (
      screen.getByRole("combobox", {
        name: "Person detector",
      }) as HTMLSelectElement
    ).value,
  ).toBe("detector");
});
