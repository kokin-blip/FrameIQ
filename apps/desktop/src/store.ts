import { create } from "zustand";
import type { JobEvent } from "./contracts";
type State = {
  jobs: Record<string, JobEvent>;
  order: string[];
  setJob: (event: JobEvent) => void;
};
export const useJobs = create<State>((set) => ({
  jobs: {},
  order: [],
  setJob: (event) =>
    set((state) => {
      const current = state.jobs[event.jobId];
      if (current && current.sequence > event.sequence) return state;
      return {
        jobs: { ...state.jobs, [event.jobId]: event },
        order: [event.jobId, ...state.order.filter((id) => id !== event.jobId)],
      };
    }),
}));
