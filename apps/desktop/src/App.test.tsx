// @vitest-environment jsdom
import { afterEach, describe, it, expect } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
afterEach(cleanup);
describe("empty workspace keyboard behavior", () => {
  it("preserves Space activation on focused buttons and keeps playback disabled without a frame", async () => {
    const user = userEvent.setup();
    render(<App />);
    const choose = screen.getByRole("button", { name: "Attach video" });
    choose.focus();
    await user.keyboard("[Space]");
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("Open the desktop app"),
    );
    choose.blur();
    await user.keyboard("[Space]");
    expect(
      screen.getByRole("button", { name: "Play muted preview" }),
    ).toHaveProperty("disabled", true);
    expect(screen.queryByRole("button", { name: "Pause preview" })).toBeNull();
    const settings = screen.getByRole("button", { name: "Settings" });
    settings.focus();
    await user.keyboard("[Space]");
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
  });
});
