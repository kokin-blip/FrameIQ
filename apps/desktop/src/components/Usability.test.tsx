// @vitest-environment jsdom
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Timeline } from "./Timeline";
import { RegionEditor } from "./RegionEditor";
import type { Workspace } from "../workspace/useWorkspace";
const api = vi.hoisted(() => ({
  frames: vi.fn(),
  thumbnail: vi.fn(),
  image: (s: string) => s,
}));
vi.mock("../client", () => ({
  api,
  message: String,
  timestamp: (s: number) => String(s),
}));
afterEach(cleanup);
it("loads exact neighbors using string ordinals and opens their identities", async () => {
  const n = "9007199254740993";
  const frame = {
    ordinal: n,
    timeSeconds: 1,
    ptsTicks: "42",
    timestampOrigin: "source",
    keyFrame: false,
    durationSeconds: 0.1,
  };
  api.frames.mockResolvedValue([frame]);
  api.thumbnail.mockResolvedValue({ imagePath: "/thumb" });
  const w = {
    sourceId: "source",
    activeSource: {
      name: "Source",
      indexGeneration: "index",
      metadata: { durationSeconds: 2 },
    },
    frame,
    playing: false,
    thumbs: [],
    show: vi.fn(),
    setPlaying: vi.fn(),
  } as unknown as Workspace;
  render(<Timeline w={w} expanded onToggle={() => {}} />);
  await waitFor(() =>
    expect(api.frames).toHaveBeenCalledWith("source", "9007199254740989", 9),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: `Seek to frame ${n}` }),
  );
  expect(w.show).toHaveBeenCalledWith("source", n);
});
it("draws and moves normalized image crops", () => {
  Object.defineProperty(window, "PointerEvent", {
    value: MouseEvent,
    configurable: true,
  });
  const change = vi.fn();
  const { rerender } = render(
    <RegionEditor
      src="/frame"
      width={100}
      height={100}
      rectangle={{ x: 0.2, y: 0.2, width: 0.3, height: 0.3 }}
      onChange={change}
    />,
  );
  const svg = screen.getByRole("img") as unknown as SVGSVGElement;
  Object.assign(svg, {
    getScreenCTM: () => ({ inverse: () => ({}) }),
    createSVGPoint: () => ({
      x: 0,
      y: 0,
      matrixTransform() {
        return this;
      },
    }),
    setPointerCapture: () => {},
  });
  fireEvent.pointerDown(svg, { clientX: 10, clientY: 20, pointerId: 1 });
  fireEvent.pointerMove(svg, { clientX: 50, clientY: 70, pointerId: 1 });
  expect(change.mock.lastCall![0]).toMatchObject({
    x: 0.1,
    y: 0.2,
    width: 0.4,
  });
  expect(change.mock.lastCall![0].height).toBeCloseTo(0.5);
  fireEvent.pointerUp(svg);
  const box = svg.querySelector('[data-crop-action="move"]')!;
  fireEvent.pointerDown(box, { clientX: 30, clientY: 30, pointerId: 1 });
  fireEvent.pointerMove(svg, { clientX: 50, clientY: 40, pointerId: 1 });
  expect(change.mock.lastCall![0].x).toBeCloseTo(0.4);
  expect(change.mock.lastCall![0].y).toBeCloseTo(0.3);
  fireEvent.pointerUp(svg);
  const corner = svg.querySelector('[data-crop-action="resize"]')!;
  fireEvent.pointerDown(corner, { clientX: 50, clientY: 50, pointerId: 1 });
  fireEvent.pointerMove(svg, { clientX: 80, clientY: 90, pointerId: 1 });
  expect(change.mock.lastCall![0].width).toBeCloseTo(0.6);
  expect(change.mock.lastCall![0].height).toBeCloseTo(0.7);
  fireEvent.pointerUp(svg);
  rerender(
    <RegionEditor
      src="/frame"
      width={100}
      height={100}
      rectangle={{ x: 0, y: 0, width: 1, height: 1 }}
      onChange={change}
    />,
  );
  fireEvent.pointerDown(svg.querySelector('[data-crop-action="move"]')!, {
    clientX: 20,
    clientY: 20,
    pointerId: 1,
  });
  fireEvent.pointerMove(svg, { clientX: 60, clientY: 70, pointerId: 1 });
  expect(change.mock.lastCall![0].x).toBeCloseTo(0.2);
  expect(change.mock.lastCall![0].width).toBeCloseTo(0.4);
});
