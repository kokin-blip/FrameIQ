// @vitest-environment jsdom
import {
  render,
  screen,
  waitFor,
  fireEvent,
  cleanup,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FashionStudio } from "./FashionStudio";
import { useJobs } from "../store";
import type { Workspace } from "../workspace/useWorkspace";
const mock = vi.hoisted(() => ({
  media: vi.fn(),
  prepare: vi.fn(),
  review: vi.fn(),
  run: vi.fn(),
  discard: vi.fn(),
  items: vi.fn(),
  groups: vi.fn(),
  photos: vi.fn(),
  candidates: vi.fn(),
  models: vi.fn(),
  saveItem: vi.fn(),
}));
vi.mock("../client", () => ({
  desktop: true,
  api: {
    image: (s: string) => s,
    subscriptionStatus: async () => ({
      activeAccountId: "account-a",
      accounts: [],
      signingIn: false,
      error: null,
    }),
    testCloudConnection: (...a: unknown[]) => mock.models(...a),
    discoverRuntime: async () => ({ models: [] }),
    cancel: async () => {},
  },
  message: (e: unknown) => (e as Error).message,
}));
vi.mock("../fashionClient", () => ({
  fashionApi: {
    history: async () => [],
    photos: (...a: unknown[]) => mock.photos(...a),
    items: (...a: unknown[]) => mock.items(...a),
    groups: (...a: unknown[]) => mock.groups(...a),
    looks: async () => [],
    exports: async () => [],
    capabilities: async () => [],
    media: (...a: unknown[]) => mock.media(...a),
    prepare: (...a: unknown[]) => mock.prepare(...a),
    review: (...a: unknown[]) => mock.review(...a),
    run: (...a: unknown[]) => mock.run(...a),
    discard: (...a: unknown[]) => mock.discard(...a),
    candidates: (...a: unknown[]) => mock.candidates(...a),
    saveItem: (...a: unknown[]) => mock.saveItem(...a),
  },
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
afterEach(cleanup);
function workspace() {
  return {
    project: { id: "project", name: "Project" },
    image: null,
    sourceId: null,
    selected: [],
    setView: vi.fn(),
  } as unknown as Workspace;
}
beforeEach(() => {
  vi.clearAllMocks();
  useJobs.setState({ jobs: {}, order: [] });
  mock.photos.mockResolvedValue([
    {
      id: "photo",
      projectId: "project",
      revision: "photo-r",
      name: "Photo",
      width: 100,
      height: 200,
      imagePath: "/photo.png",
    },
  ]);
  mock.items.mockResolvedValue([]);
  mock.groups.mockResolvedValue([]);
  mock.candidates.mockResolvedValue([]);
  mock.media.mockResolvedValue({
    path: "/photo.png",
    sha256: "photo-hash",
    width: 100,
    height: 200,
  });
  mock.models.mockResolvedValue({ models: ["vision-model"], modelLabels: {} });
  mock.discard.mockResolvedValue(undefined);
});
it("opens a real photo revision without inventing a frame identity", async () => {
  render(<FashionStudio w={workspace()} />);
  await screen.findByText("Photo revision");
  await waitFor(() =>
    expect(mock.media).toHaveBeenCalledWith("project", {
      kind: "photo",
      revision: "photo-r",
    }),
  );
  expect(screen.queryByText(/Exact frame/)).toBeNull();
});
it("requires loaded previews and consent, and invalidates review after changing inputs", async () => {
  mock.prepare.mockResolvedValue({
    jobId: "review",
    stage: "fashion_preparing",
    status: "running",
    resultIds: [],
    completedUnits: "0",
    totalUnits: "1",
  });
  mock.review.mockResolvedValue({
    id: "review",
    request: { runtime: null, task: "detect" },
    images: [
      { path: "/review.jpg", sha256: "review-hash", width: 100, height: 200 },
    ],
  });
  render(<FashionStudio w={workspace()} />);
  await screen.findByText("Photo revision");
  fireEvent.click(screen.getByText("Load available models"));
  await waitFor(() =>
    expect((screen.getByLabelText("Model") as HTMLSelectElement).value).toBe(
      "vision-model",
    ),
  );
  fireEvent.click(screen.getByText("Prepare image review"));
  await waitFor(() => expect(mock.prepare).toHaveBeenCalled());
  useJobs.getState().setJob({
    schemaVersion: 1,
    jobId: "review",
    sequence: 1,
    status: "succeeded",
    stage: "fashion_preparing",
    completedUnits: "1",
    totalUnits: "1",
    messageCode: "READY",
    resultIds: ["review"],
    error: null,
  });
  const send = await screen.findByText("Send reviewed images");
  expect((send as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(
    screen.getByLabelText("I approve this image-analysis request."),
  );
  expect((send as HTMLButtonElement).disabled).toBe(true);
  fireEvent.load(screen.getByAltText("Exact image prepared for AI analysis"));
  expect((send as HTMLButtonElement).disabled).toBe(false);
  fireEvent.change(screen.getByLabelText("Task"), {
    target: { value: "attributes" },
  });
  await waitFor(() =>
    expect(screen.queryByText("Send reviewed images")).toBeNull(),
  );
  expect(mock.run).not.toHaveBeenCalled();
  expect(mock.discard).toHaveBeenCalledWith("review");
});
it("draws a user-owned item with exact photo provenance", async () => {
  mock.saveItem.mockResolvedValue({
    jobId: "save",
    stage: "item_correction",
    status: "queued",
    resultIds: [],
    completedUnits: "0",
    totalUnits: null,
  });
  render(<FashionStudio w={workspace()} />);
  await screen.findByText("Photo revision");
  await waitFor(() =>
    expect((screen.getByText("Draw item") as HTMLButtonElement).disabled).toBe(
      false,
    ),
  );
  fireEvent.click(screen.getByText("Draw item"));
  fireEvent.change(screen.getByLabelText("Category", { selector: "select" }), {
    target: { value: "dress" },
  });
  fireEvent.change(screen.getByLabelText("Attributes · name: value"), {
    target: { value: "color: blue" },
  });
  fireEvent.click(screen.getByText("Save correction"));
  await waitFor(() => expect(mock.saveItem).toHaveBeenCalled());
  expect(mock.saveItem.mock.calls[0][0]).toMatchObject({
    media: { kind: "photo", revision: "photo-r" },
    category: "dress",
    attributes: [{ name: "color", value: "blue", origin: "user" }],
  });
});
