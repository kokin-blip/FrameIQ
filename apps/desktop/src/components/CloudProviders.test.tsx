// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  within,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CloudProviders } from "./CloudProviders";
const mocks = vi.hoisted(() => ({
  status: vi.fn(),
  save: vi.fn(),
  remove: vi.fn(),
  test: vi.fn(),
}));
vi.mock("../client", () => ({
  desktop: true,
  message: (e: unknown) => String(e),
  api: {
    cloudKeyStatus: mocks.status,
    saveCloudKey: mocks.save,
    removeCloudKey: mocks.remove,
    testCloudConnection: mocks.test,
  },
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
describe("cloud credentials", () => {
  it("does not connect on mount or key save, clears the input and removes a saved key", async () => {
    mocks.status.mockImplementation(async (provider) => ({
      provider,
      saved: false,
    }));
    mocks.save.mockResolvedValue(undefined);
    mocks.remove.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<CloudProviders />);
    const provider = within(screen.getByRole("region", { name: "OpenAI" }));
    await provider.findByText("No key saved.");
    expect(mocks.test).not.toHaveBeenCalled();
    expect(
      provider.getByRole("button", { name: "Test connection" }),
    ).toHaveProperty("disabled", true);
    const input = provider.getByLabelText("API key");
    expect(input).toHaveProperty("type", "password");
    await user.type(input, "fake-test-key");
    await user.click(provider.getByRole("button", { name: "Save key" }));
    await provider.findByText("Key saved.");
    expect(mocks.save).toHaveBeenCalledWith("openai", "fake-test-key");
    expect(input).toHaveProperty("value", "");
    expect(mocks.test).not.toHaveBeenCalled();
    await user.click(provider.getByRole("button", { name: "Remove key" }));
    await provider.findByText("Key removed.");
    expect(mocks.remove).toHaveBeenCalledWith("openai");
    expect(
      provider.getByRole("button", { name: "Test connection" }),
    ).toHaveProperty("disabled", true);
  });
  it("tests only the chosen provider after an explicit action and reports partial pagination", async () => {
    mocks.status.mockImplementation(async (provider) => ({
      provider,
      saved: true,
    }));
    mocks.test.mockResolvedValue({
      provider: "gemini",
      modelCount: 100,
      moreAvailable: true,
    });
    const user = userEvent.setup();
    render(<CloudProviders />);
    const provider = within(screen.getByRole("region", { name: "Gemini" }));
    await waitFor(() =>
      expect(
        provider.getByRole("button", { name: "Test connection" }),
      ).toHaveProperty("disabled", false),
    );
    expect(mocks.test).not.toHaveBeenCalled();
    await user.click(provider.getByRole("button", { name: "Test connection" }));
    await provider.findByText(
      "Connected. 100 models listed on the first page.",
    );
    expect(mocks.test).toHaveBeenCalledWith("gemini");
    expect(mocks.test).toHaveBeenCalledTimes(1);
  });
  it("shows credential-store errors without presenting the key as absent", async () => {
    mocks.status.mockRejectedValue(
      new Error("Unlock OS credential storage and try again."),
    );
    render(<CloudProviders />);
    const provider = within(screen.getByRole("region", { name: "OpenAI" }));
    await provider.findByRole("alert");
    expect(provider.getByText("Key status unavailable.")).toBeTruthy();
    expect(provider.queryByText("No key saved.")).toBeNull();
  });
});
