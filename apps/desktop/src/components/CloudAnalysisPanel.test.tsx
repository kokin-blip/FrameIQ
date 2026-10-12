// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CloudAnalysisPanel } from "./CloudAnalysisPanel";
import { useJobs } from "../store";
import type { Workspace } from "../workspace/useWorkspace";
import type { CloudReviewV1, JobEvent, Selection } from "../contracts";
const mocks = vi.hoisted(() => ({
  prepare: vi.fn(),
  review: vi.fn(),
  discard: vi.fn(),
  send: vi.fn(),
  result: vi.fn(),
  list: vi.fn(),
  cancel: vi.fn(),
  select: vi.fn(),
}));
vi.mock("../client", () => ({
  message: (e: unknown) => String(e),
  timestamp: (n: number) => String(n),
  api: {
    cloudKeyStatus: async (provider: string) => ({ provider, saved: true }),
    prepareCloudReview: mocks.prepare,
    cloudReview: mocks.review,
    discardCloudReview: mocks.discard,
    analyzeCloudReview: mocks.send,
    cloudResult: mocks.result,
    testCloudConnection: mocks.list,
    cancel: mocks.cancel,
    image: (path: string) => path,
    select: mocks.select,
  },
}));
const reference = {
  sourceRevisionId: "revision",
  streamIndex: 0,
  indexGeneration: "index",
  presentationOrdinal: "9007199254740993",
  ptsTicks: "1",
  timeBaseNumerator: 1,
  timeBaseDenominator: 1000,
  timestampOrigin: "source" as const,
};
const frame: Selection = {
  sourceId: "source",
  frameRef: reference,
  timeSeconds: 1,
};
const w = {
  sourceId: "source",
  image: {
    frameRef: reference,
    timeSeconds: 1,
    imagePath: "/frame.png",
    width: 100,
    height: 100,
  },
  selected: [
    frame,
    {
      ...frame,
      timeSeconds: 2,
      frameRef: { ...reference, presentationOrdinal: "2" },
    },
  ],
  sources: [{ id: "source", name: "source.mkv" }],
  setView: vi.fn(),
  openSelection: vi.fn(),
} as unknown as Workspace;
const packet: CloudReviewV1 = {
  id: "review",
  provider: "openai",
  model: "vision",
  task: "describe",
  goal: "outfit",
  images: [
    {
      id: "image_1",
      input: { frame, region: null },
      path: "/exact-reviewed.jpg",
      sha256: "hash",
      width: 100,
      height: 100,
      bytes: 2048,
    },
  ],
};
const event = (
  stage: string,
  status: JobEvent["status"],
  id = "review",
): JobEvent => ({
  schemaVersion: 1,
  jobId: id,
  sequence: status === "queued" ? 0 : 2,
  status,
  stage,
  completedUnits: "1",
  totalUnits: "1",
  messageCode: "READY",
  resultIds: [],
  error: null,
});
afterEach(() => {
  cleanup();
  useJobs.setState({ jobs: {}, order: [] });
  vi.resetAllMocks();
});
async function startReview() {
  const user = userEvent.setup();
  mocks.prepare.mockResolvedValue(event("cloud_preparing", "queued"));
  mocks.review.mockResolvedValue(packet);
  mocks.discard.mockResolvedValue(undefined);
  render(<CloudAnalysisPanel w={w} />);
  await user.type(screen.getByLabelText("Image-capable model ID"), "vision");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Review images" }),
    ).toHaveProperty("disabled", false),
  );
  await user.click(screen.getByRole("button", { name: "Review images" }));
  act(() => useJobs.getState().setJob(event("cloud_preparing", "succeeded")));
  await screen.findByRole("img", { name: "Upload preview image_1" });
  return user;
}
describe("cloud image review", () => {
  it("does not send before exact previews load and explicit consent, preserving large ordinals", async () => {
    const user = await startReview();
    expect(mocks.send).not.toHaveBeenCalled();
    const consent = screen.getByRole("checkbox", { name: /Allow sending/ });
    expect(consent).toHaveProperty("disabled", true);
    fireEvent.load(screen.getByRole("img", { name: "Upload preview image_1" }));
    expect(
      screen.getByRole("button", { name: "Send to OpenAI" }),
    ).toHaveProperty("disabled", true);
    await user.click(consent);
    mocks.send.mockResolvedValue(event("cloud_analysis", "queued", "network"));
    await user.click(screen.getByRole("button", { name: "Send to OpenAI" }));
    expect(mocks.send).toHaveBeenCalledWith("review", true);
    expect(
      mocks.prepare.mock.calls[0][0].images[0].frame.frameRef
        .presentationOrdinal,
    ).toBe("9007199254740993");
    expect(mocks.list).not.toHaveBeenCalled();
  });
  it("invalidates an approved review after model edits without automatically transmitting or retrying", async () => {
    const user = await startReview();
    fireEvent.load(screen.getByRole("img", { name: "Upload preview image_1" }));
    await user.click(screen.getByRole("checkbox", { name: /Allow sending/ }));
    await user.type(screen.getByLabelText("Image-capable model ID"), "-other");
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Send to OpenAI" }),
      ).toBeNull(),
    );
    expect(mocks.send).not.toHaveBeenCalled();
    expect(mocks.discard).toHaveBeenCalledWith("review");
  });
  it("shows uncertainty and restores the original exact frame from completed suggestions", async () => {
    const user = await startReview();
    fireEvent.load(screen.getByRole("img", { name: "Upload preview image_1" }));
    await user.click(screen.getByRole("checkbox", { name: /Allow sending/ }));
    mocks.send.mockResolvedValue(event("cloud_analysis", "queued", "network"));
    mocks.result.mockResolvedValue({
      jobId: "network",
      review: packet,
      actualModel: "actual-vision",
      inputTokens: 25,
      outputTokens: 30,
      assessments: [
        {
          imageId: "image_1",
          rank: 1,
          observed: ["Blue top"],
          uncertain: ["Material unknown"],
          reason: "Clear target view",
        },
      ],
    });
    mocks.select.mockResolvedValue(undefined);
    await user.click(screen.getByRole("button", { name: "Send to OpenAI" }));
    act(() =>
      useJobs
        .getState()
        .setJob(event("cloud_analysis", "succeeded", "network")),
    );
    await screen.findByText("AI suggestions · unverified");
    expect(screen.getByText("Material unknown")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Open exact frame" }));
    expect(w.openSelection).toHaveBeenCalledWith(frame);
    await user.click(screen.getByRole("button", { name: "Add frame" }));
    expect(mocks.select).toHaveBeenCalledWith(
      "source",
      "9007199254740993",
      true,
    );
  });
  it("requires explicitly chosen frames for ranking", async () => {
    mocks.discard.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<CloudAnalysisPanel w={w} />);
    await user.type(screen.getByLabelText("Image-capable model ID"), "vision");
    await user.selectOptions(screen.getByLabelText("Task"), "rank");
    expect(
      screen.getByRole("button", { name: "Review images" }),
    ).toHaveProperty("disabled", true);
    for (const checkbox of screen.getAllByRole("checkbox", {
      name: /source.mkv/,
    }))
      await user.click(checkbox);
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Review images" }),
      ).toHaveProperty("disabled", false),
    );
    expect(mocks.send).not.toHaveBeenCalled();
    expect(mocks.prepare).not.toHaveBeenCalled();
  });
});
