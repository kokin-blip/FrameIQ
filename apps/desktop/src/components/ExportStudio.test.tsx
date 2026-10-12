// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ExportStudio } from "./ExportStudio";
import { useJobs } from "../store";
import type { Workspace } from "../workspace/useWorkspace";
import type { ExportPlanV1, JobEvent } from "../contracts";

const api = vi.hoisted(() => ({ exportPlans: vi.fn() }));
vi.mock("../client", () => ({ api, message: String }));
const plan: ExportPlanV1 = {
  id: "own-plan",
  projectId: "own",
  destination: "/owned/export",
  active: false,
  options: { format: "png", jpegQuality: 95, colorMode: "sdr" },
  items: [],
};
function job(id: string, destination: string): JobEvent {
  return {
    schemaVersion: 1,
    jobId: id,
    sequence: 1,
    status: "succeeded",
    stage: "exporting",
    completedUnits: "2",
    totalUnits: "2",
    messageCode: "EXPORT_COMPLETE",
    resultIds: [destination],
    error: null,
  };
}
function workspace(id: string) {
  return {
    project: { id },
    selected: [],
    sources: [],
    format: "png",
    quality: 95,
    colorMode: "sdr",
    sourceColor: "",
    activeJobs: [],
    setError: vi.fn(),
    setView: vi.fn(),
    lastJob: job("foreign", "/foreign/export"),
  } as unknown as Workspace;
}
beforeEach(() => {
  api.exportPlans.mockReset().mockResolvedValue([plan]);
  useJobs.setState({ jobs: {}, order: [] });
  useJobs.getState().setJob(job("own-resume", plan.destination));
  useJobs.getState().setJob(job("foreign", "/foreign/export"));
});
afterEach(cleanup);
it("shows the active project's resumed outcome rather than the latest global export", async () => {
  render(<ExportStudio w={workspace("own")} images={{}} />);
  const outcome = await screen.findByRole("region", { name: "Export outcome" });
  expect(outcome.textContent).toContain(plan.destination);
  expect(outcome.textContent).not.toContain("/foreign/export");
});
it("hides previous-project plans and outcomes immediately while the new query is pending", async () => {
  const ui = render(<ExportStudio w={workspace("own")} images={{}} />);
  await screen.findByRole("region", { name: "Export outcome" });
  api.exportPlans.mockImplementation(() => new Promise(() => {}));
  ui.rerender(<ExportStudio w={workspace("another")} images={{}} />);
  expect(screen.queryByRole("region", { name: "Export outcome" })).toBeNull();
  expect(screen.queryByRole("region", { name: "Saved export" })).toBeNull();
});
it("does not show a foreign outcome in a project with no export plans", async () => {
  api.exportPlans.mockResolvedValue([]);
  render(<ExportStudio w={workspace("own")} images={{}} />);
  expect(screen.queryByRole("region", { name: "Export outcome" })).toBeNull();
});
