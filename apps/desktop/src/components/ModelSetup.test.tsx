// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ModelSetup } from "./ModelSetup";
import { Tutorial } from "./Tutorial";
import { useJobs, usePreferences } from "../store";
import type { ModelManifestV1 } from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";
const mock = vi.hoisted(() => ({
  catalog: vi.fn(),
  models: vi.fn(),
  statuses: vi.fn(),
  retry: vi.fn(),
  download: vi.fn(),
  jobs: vi.fn(),
  cancel: vi.fn(),
}));
vi.mock("../client", () => ({
  message: (e: Error) => e.message,
  api: {
    modelCatalog: mock.catalog,
    models: mock.models,
    modelSetupStatus: mock.statuses,
    retryModelValidation: mock.retry,
    downloadModel: mock.download,
    jobs: mock.jobs,
    cancel: mock.cancel,
    hardware: async () => ({
      architecture: "aarch64",
      availableMemoryBytes: "8589934592",
      totalMemoryBytes: "17179869184",
      aiBudgetBytes: "4294967296",
    }),
  },
}));
const packs = [
  {
    id: "person",
    name: "Person detector",
    task: "person_detector",
    byteSize: "11990159",
  },
  { id: "pose", name: "Body pose", task: "pose", byteSize: "5557238" },
  { id: "face", name: "Face detector", task: "yunet", byteSize: "232589" },
].map((m) => ({
  ...m,
  sourceUrl: "https://example.com/model.onnx",
  license: "Apache-2.0",
  licenseUrl: "https://example.com/LICENSE",
  sha256: "a".repeat(64),
})) as ModelManifestV1[];
const event = (id: string, status = "succeeded") => ({
  jobId: id,
  status,
  stage: "model_install",
  sequence: 1,
  resultIds: [],
  completedUnits: "0",
  totalUnits: null,
  error: null,
});
beforeEach(() => {
  mock.statuses.mockReset().mockImplementation(async () => {
    const models = await mock.models();
    return packs.map((p) => ({
      schemaVersion: 1,
      manifestId: p.id,
      fingerprint: p.id,
      state: models.some((m: any) => m.manifest.task === p.task)
        ? "ready"
        : "uninstalled",
      installationIds: [],
      downloaded: false,
      job: null,
      error: null,
    }));
  });
  mock.retry.mockReset();
  mock.catalog.mockReset().mockResolvedValue(packs);
  mock.models.mockReset().mockResolvedValue([]);
  mock.download
    .mockReset()
    .mockImplementation(async (m: ModelManifestV1) => event(m.id));
  mock.jobs.mockReset().mockResolvedValue([]);
  mock.cancel.mockReset().mockResolvedValue(undefined);
  useJobs.setState({ jobs: {}, order: [] });
});
afterEach(cleanup);
it("requires review before installing selected packs and enforces pose's detector dependency", async () => {
  const user = userEvent.setup();
  render(<ModelSetup visible onFinish={() => {}} />);
  const person = await screen.findByRole("checkbox", {
    name: /Person detector/,
  });
  await user.click(person);
  expect(screen.getByRole("checkbox", { name: /Body pose/ })).toHaveProperty(
    "disabled",
    true,
  );
  await user.click(person);
  await user.click(screen.getByRole("button", { name: "Review downloads" }));
  expect(
    screen.getByRole("button", { name: "Download selected models" }),
  ).toHaveProperty("disabled", true);
  expect(mock.download).not.toHaveBeenCalled();
  await user.click(screen.getByRole("checkbox", { name: /I reviewed/ }));
  await user.click(
    screen.getByRole("button", { name: "Download selected models" }),
  );
  await waitFor(() => expect(mock.download).toHaveBeenCalledTimes(2));
  expect(mock.download.mock.calls.map(([m]) => m.id)).toEqual([
    "person",
    "face",
  ]);
  expect(
    await screen.findByRole("button", { name: "Finish setup" }),
  ).toHaveProperty("disabled", false);
});
it("cancels the active download without dispatching the remaining queue", async () => {
  const user = userEvent.setup();
  let stopped = false;
  mock.download.mockImplementation(async (m: ModelManifestV1) =>
    event(m.id, "running"),
  );
  mock.cancel.mockImplementation(async () => {
    stopped = true;
  });
  mock.jobs.mockImplementation(async () => [
    event("person", stopped ? "cancelled" : "running"),
  ]);
  render(<ModelSetup visible onFinish={() => {}} />);
  await user.click(
    await screen.findByRole("button", { name: "Review downloads" }),
  );
  await user.click(screen.getByRole("checkbox", { name: /I reviewed/ }));
  await user.click(
    screen.getByRole("button", { name: "Download selected models" }),
  );
  await waitFor(() => expect(mock.download).toHaveBeenCalledTimes(1));
  await user.click(screen.getByRole("button", { name: "Cancel downloads" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Finish setup" })).toHaveProperty(
      "disabled",
      false,
    ),
  );
  expect(mock.cancel).toHaveBeenCalledWith("person");
  expect(mock.download).toHaveBeenCalledTimes(1);
});
it("retries failed packs while preserving completed installations", async () => {
  const user = userEvent.setup();
  let calls = 0;
  const ready: string[] = [];
  mock.models.mockImplementation(async () =>
    ready.map((id) => ({
      id: `uuid-${id}`,
      status: "ready",
      manifest: {
        id: `uuid-${id}`,
        task: packs.find((p) => p.id === id)?.task,
      },
    })),
  );
  mock.download.mockImplementation(async (m: ModelManifestV1) => {
    calls++;
    if (m.id === "pose" && calls === 2)
      return {
        ...event(m.id, "failed"),
        error: { message: "Not enough memory" },
      };
    ready.push(m.id);
    return event(m.id);
  });
  render(<ModelSetup visible onFinish={() => {}} />);
  await user.click(
    await screen.findByRole("button", { name: "Review downloads" }),
  );
  await user.click(screen.getByRole("checkbox", { name: /I reviewed/ }));
  await user.click(
    screen.getByRole("button", { name: "Download selected models" }),
  );
  await screen.findByText("Not enough memory");
  await user.click(
    screen.getByRole("button", { name: "Retry unfinished models" }),
  );
  await waitFor(() => expect(mock.download).toHaveBeenCalledTimes(4));
  expect(mock.download.mock.calls.map(([m]) => m.id)).toEqual([
    "person",
    "pose",
    "face",
    "pose",
  ]);
});
it("provides a skippable real-workspace tutorial and persists its completion and selection preference", async () => {
  usePreferences.setState({
    setupCompleted: true,
    tutorialCompleted: false,
    clearSelectionsOnSwitch: true,
  });
  const w = {
    view: "extract",
    activeSource: { id: "clip" },
    frame: { ordinal: "0" },
    busy: false,
    selected: [{}],
    step: vi.fn(),
    setView: vi.fn(),
    importVideo: vi.fn(),
  } as unknown as Workspace;
  const user = userEvent.setup();
  render(<Tutorial w={w} />);
  await user.click(screen.getByRole("button", { name: "Next" }));
  await user.click(screen.getByRole("button", { name: "Try next frame" }));
  expect(w.step).toHaveBeenCalledWith(1);
  await user.click(screen.getByRole("button", { name: "Next" }));
  await user.click(screen.getByRole("button", { name: "Open Export Studio" }));
  expect(w.setView).toHaveBeenCalledWith("export");
  await user.click(screen.getByRole("button", { name: "Finish tutorial" }));
  usePreferences.getState().setClearSelectionsOnSwitch(false);
  const saved = JSON.parse(localStorage.getItem("frameiq-preferences-v1")!);
  expect(saved.state.tutorialCompleted).toBe(true);
  expect(saved.state.clearSelectionsOnSwitch).toBe(false);
  expect(
    screen.queryByRole("region", { name: "Workspace tutorial" }),
  ).toBeNull();
  act(() => usePreferences.getState().replayTutorial());
  expect(screen.getByText(/1 of 3/).textContent).toContain("Attach a video");
});

it("recognizes an installed catalog pack with a different installation UUID", async () => {
  mock.statuses.mockResolvedValue(
    packs.map((p) => ({
      manifestId: p.id,
      fingerprint: p.id,
      state: "ready",
      downloaded: false,
      installationIds: [`uuid-${p.id}`],
      job: null,
      error: null,
    })),
  );
  render(<ModelSetup visible onFinish={() => {}} />);
  const person = await screen.findByRole("checkbox", {
    name: /Person detector/,
  });
  expect(person).toHaveProperty("disabled", true);
  expect(
    screen.getByRole("button", { name: "Review downloads" }),
  ).toHaveProperty("disabled", true);
  expect(mock.download).not.toHaveBeenCalled();
});

it("keeps memory-blocked downloaded packs unavailable and retries validation without downloading", async () => {
  const user = userEvent.setup();
  let ready = false;
  mock.statuses.mockImplementation(async () =>
    packs.map((p) => ({
      manifestId: p.id,
      fingerprint: p.id,
      state: p.id === "person" ? (ready ? "ready" : "blocked") : "uninstalled",
      downloaded: p.id === "person",
      installationIds: ready && p.id === "person" ? ["uuid"] : [],
      job: null,
      error: null,
    })),
  );
  mock.retry.mockImplementation(async () => {
    ready = true;
    return event("retry");
  });
  render(<ModelSetup visible onFinish={() => {}} />);
  expect(
    await screen.findByText(
      "Downloaded; validation blocked by available memory",
    ),
  ).toBeTruthy();
  await user.click(
    screen.getByRole("button", { name: "Refresh memory status" }),
  );
  await user.click(screen.getByRole("button", { name: "Retry validation" }));
  await screen.findByText("Installed and compatibility checked");
  expect(mock.retry).toHaveBeenCalledTimes(1);
  expect(mock.download).not.toHaveBeenCalled();
});
