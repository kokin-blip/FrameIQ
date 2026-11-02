import { invoke } from "@tauri-apps/api/core";
import type {
  AssetRefV1,
  FashionItemV1,
  FashionReviewRequestV1,
  FashionReviewV1,
  FeatureCapabilityV1,
  GarmentGroupV1,
  JobEvent,
  MediaRefV1,
  PhotoSourceV1,
  ProductCandidateV1,
  ProductV1,
  SavedLookV1,
  TaskPackV2,
} from "./contracts";
export const fashionApi = {
  history: (project: string) =>
    invoke<unknown[]>("fashion_history", { project }),
  ebayStatus: () => invoke<boolean>("ebay_status"),
  saveEbay: (clientId: string, clientSecret: string, approved: boolean) =>
    invoke<void>("save_ebay_credentials", { clientId, clientSecret, approved }),
  removeEbay: () => invoke<void>("remove_ebay_credentials"),
  exports: (project: string) =>
    invoke<import("./contracts").FashionExportV1[]>("fashion_exports", {
      project,
    }),
  exportLook: (
    project: string,
    id: string,
    destination: string,
    format: string,
  ) => invoke<JobEvent>("export_look", { project, id, destination, format }),
  resumeExport: (id: string) => invoke<JobEvent>("resume_look_export", { id }),
  photos: (project: string) => invoke<PhotoSourceV1[]>("photos", { project }),
  importPhoto: (project: string, path: string) =>
    invoke<JobEvent>("import_photo", { project, path }),
  media: (project: string, media: MediaRefV1) =>
    invoke<AssetRefV1>("fashion_media", { project, media }),
  items: (project: string) =>
    invoke<FashionItemV1[]>("fashion_items", { project }),
  saveItem: (item: FashionItemV1) =>
    invoke<JobEvent>("save_fashion_item", { item }),
  deleteItem: (id: string) => invoke<void>("delete_fashion_item", { id }),
  undoItem: (id: string) => invoke<FashionItemV1>("undo_fashion_item", { id }),
  groups: (project: string) =>
    invoke<GarmentGroupV1[]>("garment_groups", { project }),
  saveGroup: (group: GarmentGroupV1) =>
    invoke<GarmentGroupV1>("save_garment_group", { group }),
  associate: (project: string) =>
    invoke<JobEvent>("associate_garments", { project }),
  candidates: (group: string) =>
    invoke<ProductCandidateV1[]>("product_candidates", { group }),
  confirm: (
    group: string,
    id: string,
    identifier: string,
    sourceUrl: string,
    authoritative: boolean,
  ) =>
    invoke<ProductCandidateV1>("confirm_product_candidate", {
      group,
      id,
      identifier,
      sourceUrl,
      authoritative,
    }),
  looks: (project: string) => invoke<SavedLookV1[]>("saved_looks", { project }),
  saveLook: (look: SavedLookV1) => invoke<SavedLookV1>("save_look", { look }),
  deleteLook: (id: string) => invoke<void>("delete_look", { id }),
  capabilities: () => invoke<FeatureCapabilityV1[]>("fashion_capabilities"),
  modelCatalog: () => invoke<TaskPackV2[]>("fashion_model_catalog"),
  downloadPack: (id: string, reviewed: boolean) =>
    invoke<JobEvent>("download_task_pack", { id, reviewed }),
  detect: (project: string, media: MediaRefV1) =>
    invoke<JobEvent>("detect_fashion_local", { project, media }),
  refineMask: (id: string) => invoke<JobEvent>("refine_item_mask", { id }),
  catalog: () => invoke<ProductV1[]>("catalog_products"),
  importCatalog: (path: string) => invoke<JobEvent>("import_catalog", { path }),
  searchCatalog: (group: string, query: string) =>
    invoke<ProductCandidateV1[]>("search_catalog", { group, query }),
  readPack: (path: string) => invoke<TaskPackV2>("read_task_pack", { path }),
  importPack: (manifest: string, reviewed: boolean) =>
    invoke<JobEvent>("import_task_pack", { manifest, reviewed }),
  removePack: (id: string) => invoke<void>("remove_task_pack", { id }),
  prepare: (request: FashionReviewRequestV1) =>
    invoke<JobEvent>("prepare_fashion_review", { request }),
  review: (id: string) => invoke<FashionReviewV1>("fashion_review", { id }),
  discard: (id: string) => invoke<void>("discard_fashion_review", { id }),
  run: (id: string, consent: boolean) =>
    invoke<JobEvent>("run_fashion_review", { id, consent }),
};
