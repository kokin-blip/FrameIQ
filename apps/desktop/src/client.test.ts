import { describe, it, expect } from "vitest";
import { parseTimestamp, timestamp } from "./client";
import { useJobs } from "./store";
import type { JobEvent } from "./contracts";
describe("precision timestamp entry", () => {
  it("keeps subsecond resolution and rejects ambiguous values", () => {
    expect(parseTimestamp("01:02:03.007")).toBe(3723.007);
    expect(timestamp(3723.007)).toBe("01:02:03.007");
    expect(parseTimestamp("1.25")).toBe(1.25);
    for (const invalid of ["NaN", "-1", "00:60:00", "1:2", "foo"])
      expect(parseTimestamp(invalid)).toBeNull();
  });
});
describe("recoverable job snapshots", () => {
  it("ignores old events that arrive after a newer result", () => {
    const e: JobEvent = {
      schemaVersion: 1,
      jobId: "test",
      sequence: 5,
      status: "succeeded",
      stage: "indexing",
      completedUnits: "9",
      totalUnits: "9",
      messageCode: "READY",
      resultIds: [],
      error: null,
    };
    useJobs.getState().setJob(e);
    useJobs.getState().setJob({ ...e, sequence: 4, status: "running" });
    expect(useJobs.getState().jobs.test.status).toBe("succeeded");
  });
});

it("orders different jobs by recency rather than comparing their independent sequence numbers", () => {
  const first: JobEvent = {
    schemaVersion: 1,
    jobId: "older-export",
    sequence: 99,
    status: "succeeded",
    stage: "exporting",
    completedUnits: "5",
    totalUnits: "5",
    messageCode: "COMPLETE",
    resultIds: [],
    error: null,
  };
  useJobs.getState().setJob(first);
  useJobs
    .getState()
    .setJob({ ...first, jobId: "new-export", sequence: 0, status: "queued" });
  expect(useJobs.getState().order[0]).toBe("new-export");
});
