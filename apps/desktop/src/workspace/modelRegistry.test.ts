import { beforeEach, expect, it, vi } from "vitest";
const mock = vi.hoisted(() => ({
  models: vi.fn(),
  catalog: vi.fn(),
  statuses: vi.fn(),
}));
vi.mock("../client", () => ({
  api: {
    models: mock.models,
    modelCatalog: mock.catalog,
    modelSetupStatus: mock.statuses,
  },
}));
import { useModelRegistry } from "./modelRegistry";
beforeEach(() => {
  useModelRegistry.setState({ models: [], catalog: [], statuses: [] });
  mock.models.mockReset().mockResolvedValue([]);
  mock.catalog.mockReset().mockResolvedValue([{ id: "catalog-pack" }]);
  mock.statuses
    .mockReset()
    .mockResolvedValue([{ manifestId: "catalog-pack", state: "uninstalled" }]);
});
it("concurrent first-load consumers receive the same committed catalog snapshot", async () => {
  let resolve!: (value: []) => void;
  mock.models.mockReturnValue(
    new Promise<[]>((r) => {
      resolve = r;
    }),
  );
  const first = useModelRegistry.getState().refresh();
  const setup = useModelRegistry.getState().refresh();
  expect(mock.catalog).toHaveBeenCalledTimes(1);
  resolve([]);
  await Promise.all([first, setup]);
  expect(useModelRegistry.getState().catalog[0].id).toBe("catalog-pack");
});
it("activation requests a new snapshot when an earlier read is still in flight", async () => {
  let resolve!: (value: []) => void;
  mock.models.mockReturnValueOnce(
    new Promise<[]>((r) => {
      resolve = r;
    }),
  );
  const first = useModelRegistry.getState().refresh();
  mock.models.mockResolvedValue([{ id: "installed-uuid", status: "ready" }]);
  const activated = useModelRegistry.getState().refresh(true);
  resolve([]);
  await Promise.all([first, activated]);
  expect(useModelRegistry.getState().models[0].id).toBe("installed-uuid");
  expect(mock.models).toHaveBeenCalledTimes(2);
});
