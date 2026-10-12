// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { FrameCards, FrameCollection } from "./FrameCollection";
import type { Selection } from "../contracts";
import type { Workspace } from "../workspace/useWorkspace";
vi.mock("./PresetPanel", () => ({ PresetPanel: () => <p>Preset controls</p> }));
vi.mock("../client", () => ({
  timestamp: (s: number) => String(s),
  api: {
    thumbnail: vi.fn().mockRejectedValue(new Error("Unavailable")),
    image: (s: string) => s,
  },
}));
afterEach(cleanup);
it("pages large collections and retains exact identities for opening/removal", async () => {
  const selected = Array.from(
    { length: 205 },
    (_, n) =>
      ({
        sourceId: "source",
        timeSeconds: n,
        frameRef: {
          indexGeneration: "index",
          presentationOrdinal: (9007199254740993n + BigInt(n)).toString(),
        },
      }) as Selection,
  );
  const w = {
    selected,
    sources: [{ id: "source", name: "a long filename.mkv" }],
    openSelection: vi.fn(),
    selectionBusy: false,
  } as unknown as Workspace;
  const remove = vi.fn();
  render(<FrameCards w={w} images={{}} onRemove={remove} />);
  expect(screen.getAllByRole("button", { name: /^Open frame/ })).toHaveLength(
    100,
  );
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Next" }));
  await user.click(
    screen.getByRole("button", {
      name: `Open frame ${selected[100].frameRef.presentationOrdinal} from a long filename.mkv`,
    }),
  );
  expect(w.openSelection).toHaveBeenCalledWith(selected[100]);
  await user.click(
    screen.getByRole("button", {
      name: `Remove frame ${selected[100].frameRef.presentationOrdinal} from a long filename.mkv`,
    }),
  );
  expect(remove).toHaveBeenCalledWith(selected[100]);
  await user.click(screen.getByRole("button", { name: "Next" }));
  expect(screen.getAllByRole("button", { name: /^Open frame/ })).toHaveLength(
    5,
  );
});
it("disables bulk mutations while busy and shows presets inside Selections", async () => {
  const w = {
    selected: [],
    sources: [],
    selectionBusy: true,
    frame: { ordinal: "0" },
    activeSource: { status: "ready" },
    isSelected: false,
  } as unknown as Workspace;
  render(<FrameCollection w={w} images={{}} />);
  expect(
    (
      screen.getByRole("button", {
        name: "Add current frame",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("button", { name: "Select all" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("button", { name: "Remove all" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect(screen.getByRole("status").textContent).toBe("Updating selections…");
  await userEvent.setup().click(screen.getByText("Auto-select with a preset"));
  expect(await screen.findByText("Preset controls")).toBeTruthy();
});
