import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { FashionTaskV1 } from "./contracts";
export type FashionEngine = {
  runtime: "" | "ollama" | "lm_studio";
  model: string;
  port: number;
};
const defaults: FashionEngine = { runtime: "", model: "", port: 11434 };
export const useFashionPreferences = create<{
  tasks: Record<FashionTaskV1, FashionEngine>;
  setTask: (task: FashionTaskV1, engine: FashionEngine) => void;
}>()(
  persist(
    (set) => ({
      tasks: {
        detect: { ...defaults },
        attributes: { ...defaults },
        research: { ...defaults },
      },
      setTask: (task, engine) =>
        set((s) => ({ tasks: { ...s.tasks, [task]: engine } })),
    }),
    { name: "frameiq-fashion-routing-v1", version: 1 },
  ),
);
export const useFashionTab = create<{
  tab: string;
  setTab: (tab: string) => void;
}>((set) => ({ tab: "items", setTab: (tab) => set({ tab }) }));
