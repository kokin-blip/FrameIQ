import { create } from "zustand";
import { persist } from "zustand/middleware";
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

export const useSettingsTab = create<{
  tab: "models" | "cloud" | "general";
  setTab: (tab: "models" | "cloud" | "general") => void;
}>((set) => ({ tab: "models", setTab: (tab) => set({ tab }) }));

export const usePreferences = create<{
  setupCompleted: boolean;
  tutorialCompleted: boolean;
  clearSelectionsOnSwitch: boolean;
  completeSetup: () => void;
  completeTutorial: () => void;
  replayTutorial: () => void;
  setClearSelectionsOnSwitch: (enabled: boolean) => void;
}>()(
  persist(
    (set) => ({
      setupCompleted: false,
      tutorialCompleted: false,
      clearSelectionsOnSwitch: true,
      completeSetup: () => set({ setupCompleted: true }),
      completeTutorial: () => set({ tutorialCompleted: true }),
      replayTutorial: () => set({ tutorialCompleted: false }),
      setClearSelectionsOnSwitch: (enabled) =>
        set({ clearSelectionsOnSwitch: enabled }),
    }),
    {
      name: "frameiq-preferences-v1",
      partialize: ({
        setupCompleted,
        tutorialCompleted,
        clearSelectionsOnSwitch,
      }) => ({
        setupCompleted,
        tutorialCompleted,
        clearSelectionsOnSwitch,
      }),
    },
  ),
);
