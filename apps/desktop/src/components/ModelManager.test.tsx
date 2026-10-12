// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ModelManager } from "./ModelManager";
const state = vi.hoisted(() => ({
  open: vi.fn(),
  download: vi.fn(),
  discover: vi.fn(),
  probe: vi.fn(),
  token: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: state.open }));
vi.mock("../client", () => ({
  desktop: true,
  message: (e: unknown) => String(e),
  api: {
    modelCatalog: async () => [],
    hardware: async () => ({
      architecture: "aarch64",
      logicalCores: 12,
      totalMemoryBytes: "25769803776",
      availableMemoryBytes: "6000000000",
      aiBudgetBytes: "4294967296",
      workerThreads: 4,
    }),
    models: async () => [],
    benchmarks: async () => [],
    onJob: async () => () => {},
    readModelManifest: async () => ({
      name: "Test model",
      task: "yunet",
      byteSize: "232589",
      license: "MIT",
      sourceUrl: "https://example.test/model",
      licenseUrl: "https://example.test/license",
      sha256: "abc",
    }),
    downloadModel: state.download,
    discoverRuntime: state.discover,
    probeRuntime: state.probe,
    saveRuntimeToken: state.token,
  },
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
describe("model controls", () => {
  it("requires review before initiating a download", async () => {
    const user = userEvent.setup();
    state.open.mockResolvedValue("/manifest.json");
    render(<ModelManager />);
    await user.click(screen.getByText("Advanced model import"));
    await user.click(
      screen.getByRole("button", { name: "Open model manifest" }),
    );
    const button = await screen.findByRole("button", {
      name: "Download and validate",
    });
    expect(button).toHaveProperty("disabled", true);
    await user.click(screen.getByRole("checkbox"));
    await user.click(button);
    expect(state.download).toHaveBeenCalledTimes(1);
  });
  it("does not discover runtimes automatically and rejects text or remote probes", async () => {
    const user = userEvent.setup();
    state.discover.mockResolvedValue({
      runtime: "ollama",
      models: [
        { id: "text", local: true, vision: false },
        { id: "cloud", local: false, vision: true },
      ],
    });
    render(<ModelManager />);
    expect(state.discover).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "Connect and list models" }),
    );
    for (const button of await screen.findAllByRole("button", {
      name: "Test image input",
    }))
      expect(button).toHaveProperty("disabled", true);
    expect(state.probe).not.toHaveBeenCalled();
  });
});
