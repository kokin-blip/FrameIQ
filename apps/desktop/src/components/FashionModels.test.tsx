// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FashionModels } from "./FashionModels";
import { ModelSetup } from "./ModelSetup";
import { useJobs } from "../store";
import type { JobEvent, TaskPackV2 } from "../contracts";

const mock = vi.hoisted(() => ({
  capabilities: vi.fn(),
  catalog: vi.fn(),
  importLocal: vi.fn(),
  pick: vi.fn(),
  read: vi.fn(),
  install: vi.fn(),
  remove: vi.fn(),
  jobs: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mock.pick }));
vi.mock("../fashionClient", () => ({
  fashionApi: {
    capabilities: mock.capabilities,
    modelCatalog: mock.catalog,
    downloadPack: mock.install,
    readPack: mock.read,
    importPack: mock.importLocal,
    removePack: mock.remove,
  },
}));
vi.mock("../client", () => ({
  message: (e: Error) => e.message,
  api: {
    jobs: mock.jobs,
    cancel: mock.cancel,
    models: async () => [],
    modelCatalog: async () => [],
    modelSetupStatus: async () => [],
    hardware: async () => ({
      architecture: "aarch64",
      availableMemoryBytes: "8589934592",
      totalMemoryBytes: "17179869184",
      aiBudgetBytes: "4294967296",
    }),
  },
}));
const manifest = {
  schemaVersion: 2,
  id: "reviewed-hair-pack",
  task: "hair",
  adapter: "hair_segmenter_v1",
  inputs: [],
  outputs: [],
  artifacts: [
    {
      name: "model.onnx",
      byteSize: "16454717",
      sha256: "a".repeat(64),
      sourceUrl: "https://example.com/model.onnx",
    },
  ],
  license: "Apache-2.0",
  licenseUrl: "https://example.com/LICENSE",
  preprocessing: "{}",
  qualification: "reference_checked",
} satisfies TaskPackV2;
const event = (status: JobEvent["status"]): JobEvent => ({
  schemaVersion: 1,
  jobId: "hair-install",
  sequence: status === "running" ? 1 : 2,
  stage: "task_pack_import",
  status,
  resultIds: [],
  completedUnits: "0",
  totalUnits: null,
  messageCode: "TASK_PACK_READY",
  error: null,
});
beforeEach(() => {
  vi.clearAllMocks();
  mock.capabilities.mockResolvedValue([]);
  mock.catalog.mockResolvedValue(
    ["detection", "hair", "mask", "embedding"].map((task) => ({
      ...manifest,
      task,
      id: task === "hair" ? manifest.id : `reviewed-${task}-pack`,
    })),
  );
  mock.pick.mockResolvedValue("/packages/hair/manifest.json");
  mock.read.mockResolvedValue(manifest);
  mock.install.mockResolvedValue(event("succeeded"));
  mock.jobs.mockResolvedValue([]);
  mock.cancel.mockResolvedValue(undefined);
  mock.remove.mockResolvedValue(undefined);
  useJobs.setState({ jobs: {}, order: [] });
});
afterEach(cleanup);
async function chooseHair() {
  await userEvent.click(
    await screen.findByRole("button", {
      name: "Install Hair selection pack",
    }),
  );
  await screen.findByRole("heading", {
    name: "Review installation: Hair selection",
  });
}
async function review() {
  await userEvent.click(
    screen.getByRole("checkbox", { name: /I reviewed this pack/ }),
  );
}
it("explains each pack and its approximate size without starting an installation", async () => {
  render(<FashionModels onboarding />);
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Install Hair selection pack" }),
    ).toHaveProperty("disabled", false),
  );
  for (const amount of ["695 MB", "16.5 MB", "375 MB", "372 MB"])
    expect(screen.getByText(`About ${amount}`)).toBeTruthy();
  expect(screen.getByText(/Traces an item's edges/)).toBeTruthy();
  expect(
    screen.getByText(/visual match does not prove product identity/),
  ).toBeTruthy();
  expect(screen.getByText(/checks and installs it automatically/)).toBeTruthy();
  expect(mock.pick).not.toHaveBeenCalled();
  expect(mock.install).not.toHaveBeenCalled();
});
it("requires review, installs the chosen package and shows the refreshed installed state", async () => {
  render(<FashionModels onboarding />);
  await chooseHair();
  expect(document.activeElement?.textContent).toBe(
    "Review installation: Hair selection",
  );
  const install = screen.getByRole("button", { name: "Download and install" });
  expect(install).toHaveProperty("disabled", true);
  expect(mock.install).not.toHaveBeenCalled();
  mock.capabilities.mockResolvedValue([
    {
      task: "hair",
      status: "experimental",
      modelId: manifest.id,
      reason: null,
    },
  ]);
  await review();
  await userEvent.click(install);
  expect(mock.install).toHaveBeenCalledWith(manifest.id, true);
  await screen.findByRole("button", { name: "Remove Hair selection pack" });
  expect(
    screen.queryByRole("button", { name: "Install Hair selection pack" }),
  ).toBeNull();
  expect(useJobs.getState().jobs["hair-install"].status).toBe("succeeded");
});
it("keeps manual imports in Settings and resets consent when another pack is selected", async () => {
  render(<FashionModels />);
  await chooseHair();
  await review();
  await userEvent.click(
    screen.getAllByText("Import a local package instead")[0],
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Choose local Item detection package" }),
  );
  await screen.findByText(/This package is for a different feature/);
  expect(mock.importLocal).not.toHaveBeenCalled();
  await userEvent.click(
    screen.getByRole("button", { name: "Install Item cutouts pack" }),
  );
  expect(
    screen.getByRole("checkbox", { name: /I reviewed this pack/ }),
  ).toHaveProperty("checked", false);
});
it("does not show a file picker in onboarding and keeps a cancelled Settings import idle", async () => {
  mock.pick.mockResolvedValue(null);
  const view = render(<FashionModels onboarding />);
  await chooseHair();
  expect(mock.pick).not.toHaveBeenCalled();
  expect(screen.queryByText("Import a local package instead")).toBeNull();
  view.rerender(<FashionModels />);
  await userEvent.click(
    screen.getAllByText("Import a local package instead")[1],
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Choose local Hair selection package" }),
  );
  expect(mock.read).not.toHaveBeenCalled();
  expect(mock.install).not.toHaveBeenCalled();
});
it("blocks leaving onboarding during installation, supports cancellation and permits retry", async () => {
  mock.install.mockResolvedValue({
    ...event("running"),
    messageCode: "TASK_PACK_DOWNLOADING",
    completedUnits: "8000000",
    totalUnits: "16470743",
  });
  let stopped = false;
  mock.jobs.mockImplementation(async () => [
    event(stopped ? "cancelled" : "running"),
  ]);
  mock.cancel.mockImplementation(async () => {
    stopped = true;
  });
  render(<ModelSetup visible onFinish={() => {}} />);
  await chooseHair();
  await review();
  await userEvent.click(
    screen.getByRole("button", { name: "Download and install" }),
  );
  await screen.findByRole("button", { name: "Cancel installation" });
  expect(
    screen.getByRole("progressbar", { name: "Downloading Hair selection" }),
  ).toHaveProperty("value", 8000000);
  expect(
    screen.queryByRole("button", { name: "Remove Hair selection pack" }),
  ).toBeNull();
  expect(screen.getByRole("button", { name: "Finish setup" })).toHaveProperty(
    "disabled",
    true,
  );
  await userEvent.click(
    screen.getByRole("button", { name: "Cancel installation" }),
  );
  await screen.findByText(/Installation cancelled/);
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Finish setup" })).toHaveProperty(
      "disabled",
      false,
    ),
  );
  expect(mock.cancel).toHaveBeenCalledWith("hair-install");
  mock.install.mockResolvedValue(event("succeeded"));
  await userEvent.click(
    screen.getByRole("button", { name: "Retry installation" }),
  );
  await waitFor(() => expect(mock.install).toHaveBeenCalledTimes(2));
});
it("keeps installation errors actionable and recovers from an unavailable status check", async () => {
  mock.capabilities.mockRejectedValueOnce(new Error("Worker unavailable"));
  render(<FashionModels />);
  await screen.findByText(/Could not check installed packs/);
  await userEvent.click(
    screen.getByRole("button", { name: "Retry pack status" }),
  );
  await chooseHair();
  mock.install.mockResolvedValue({
    ...event("failed"),
    error: {
      code: "MEMORY_LIMIT",
      message: "Free memory and retry.",
      retryable: true,
    },
  });
  await review();
  await userEvent.click(
    screen.getByRole("button", { name: "Download and install" }),
  );
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Retry installation" }),
    ).toHaveProperty("disabled", false),
  );
  expect(screen.getAllByText("Free memory and retry.").length).toBeGreaterThan(
    0,
  );
});
it("preserves existing installations on reopening and supports removal", async () => {
  mock.capabilities.mockResolvedValue([
    {
      task: "hair",
      status: "experimental",
      modelId: manifest.id,
      reason: null,
    },
  ]);
  render(<FashionModels />);
  const remove = await screen.findByRole("button", {
    name: "Remove Hair selection pack",
  });
  expect(mock.install).not.toHaveBeenCalled();
  mock.capabilities.mockResolvedValue([]);
  await act(async () => {
    await userEvent.click(remove);
  });
  await screen.findByRole("button", { name: "Install Hair selection pack" });
  expect(mock.remove).toHaveBeenCalledWith(manifest.id);
});
