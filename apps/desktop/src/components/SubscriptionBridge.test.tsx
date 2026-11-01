// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SubscriptionBridge } from "./SubscriptionBridge";
import type { SubscriptionStatusV1 } from "../contracts";

const mocks = vi.hoisted(() => ({
  status: vi.fn(),
  start: vi.fn(),
  cancel: vi.fn(),
  select: vi.fn(),
  disconnect: vi.fn(),
  test: vi.fn(),
}));
vi.mock("../client", () => ({
  desktop: true,
  message: (e: unknown) => String(e),
  api: {
    subscriptionStatus: mocks.status,
    startSubscriptionLogin: mocks.start,
    cancelSubscriptionLogin: mocks.cancel,
    selectSubscriptionAccount: mocks.select,
    disconnectSubscription: mocks.disconnect,
    testCloudConnection: mocks.test,
  },
}));
let status: SubscriptionStatusV1;
beforeEach(() => {
  status = {
    accounts: [],
    activeAccountId: null,
    signingIn: false,
    error: null,
  };
  mocks.status.mockImplementation(async () => ({ ...status }));
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("starts and cancels browser sign-in only after an explicit action", async () => {
  mocks.start.mockImplementation(async () => {
    status.signingIn = true;
  });
  mocks.cancel.mockImplementation(async () => {
    status.signingIn = false;
  });
  const user = userEvent.setup();
  render(<SubscriptionBridge />);
  await screen.findByText("No ChatGPT subscription connected.");
  expect(mocks.start).not.toHaveBeenCalled();
  expect(mocks.test).not.toHaveBeenCalled();
  await user.click(
    screen.getByRole("button", { name: "Continue with ChatGPT" }),
  );
  await screen.findByText("Finish signing in with ChatGPT in your browser…");
  expect(mocks.start).toHaveBeenCalledWith(null);
  await user.click(screen.getByRole("button", { name: "Cancel sign-in" }));
  await screen.findByText("No ChatGPT subscription connected.");
  expect(mocks.cancel).toHaveBeenCalledTimes(1);
  expect(mocks.test).not.toHaveBeenCalled();
});

it("uses distinct saved registrations and preserves actionable remote sign-out errors", async () => {
  status = {
    activeAccountId: "first",
    signingIn: false,
    error: null,
    accounts: [
      { id: "first", label: "same@example.com · first", connected: true },
      { id: "second", label: "same@example.com · second", connected: true },
    ],
  };
  mocks.select.mockImplementation(async (id: string) => {
    status.activeAccountId = id;
  });
  mocks.disconnect.mockImplementation(async () => {
    status.accounts[1].connected = false;
    throw new Error(
      "Remote revocation was not confirmed; disconnect FrameIQ in ChatGPT Settings.",
    );
  });
  const user = userEvent.setup();
  render(<SubscriptionBridge />);
  await screen.findByText("Connected: same@example.com · first");
  await user.selectOptions(screen.getByLabelText("ChatGPT account"), "second");
  await screen.findByText("Connected: same@example.com · second");
  await user.click(screen.getByRole("button", { name: "Sign out" }));
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    expect.stringContaining("Remote revocation was not confirmed"),
  );
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Test subscription" }),
    ).toHaveProperty("disabled", true),
  );
  expect(mocks.disconnect).toHaveBeenCalledTimes(1);
});
