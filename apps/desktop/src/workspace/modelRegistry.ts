import { create } from "zustand";
import { api } from "../client";
import type {
  ModelInstallationV1,
  ModelManifestV1,
  ModelSetupStatusV1,
} from "../contracts";
type Registry = {
  models: ModelInstallationV1[];
  catalog: ModelManifestV1[];
  statuses: ModelSetupStatusV1[];
  refresh: (fresh?: boolean) => Promise<ModelInstallationV1[]>;
};
let pending: Promise<ModelInstallationV1[]> | undefined;
export const useModelRegistry = create<Registry>((set, get) => ({
  models: [],
  catalog: [],
  statuses: [],
  async refresh(fresh = false) {
    // Initial consumers share one snapshot. Mutation completions request a subsequent
    // snapshot when an in-flight read may predate activation/removal.
    if (pending) return fresh ? pending.then(() => get().refresh()) : pending;
    pending = (async () => {
      const [models, catalog, statuses] = await Promise.all([
        api.models(),
        api.modelCatalog(),
        api.modelSetupStatus(),
      ]);
      const ready = models.filter((m) => m.status === "ready");
      set({ models: ready, catalog, statuses });
      return ready;
    })();
    try {
      return await pending;
    } finally {
      pending = undefined;
    }
  },
}));
export function setupLabel(s?: ModelSetupStatusV1): string {
  if (!s) return "Not installed";
  if (s.state === "ready") return "Installed and compatibility checked";
  if (s.state === "blocked" && s.downloaded)
    return "Downloaded; validation blocked by available memory";
  if (s.state === "validating") return "Validating and activating";
  if (s.state === "downloading") return "Downloading";
  if (s.downloaded) return `Downloaded; awaiting validation (${s.state})`;
  return (
    (
      {
        failed: "Installation failed",
        cancelled: "Cancelled",
        interrupted: "Interrupted",
        uninstalled: "Not installed",
      } as Record<string, string>
    )[s.state] ?? s.state
  );
}
