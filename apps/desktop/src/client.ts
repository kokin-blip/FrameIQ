import { invoke, convertFileSrc, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CloudReviewRequestV1,
  CloudReviewV1,
  CloudAnalysisResultV1,
  CloudProviderV1,
  CloudCredentialStatusV1,
  CloudConnectionV1,
  PresetV1,
  PresetExecutionV1,
  PresetResultV1,
  FaceFrameV1,
  PersonFrameV1,
  SubjectChoiceV1,
  PoseFrameV1,
  RegionRectV1,
  GuidedRegionV1,
  ModelManifestV1,
  ModelInstallationV1,
  ModelBenchmarkV1,
  HardwareProfileV1,
  LocalRuntimeReportV1,
  Project,
  SourceSummary,
  IndexedFrame,
  FrameView,
  Selection,
  JobEvent,
  ExportOptions,
  ExportPlanV1,
  ExportAttemptSummaryV1,
  RecommendationSetV1,
  RankingConfigV1,
} from "./contracts";
export const desktop = isTauri();
function call<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop)
    return Promise.reject({
      code: "DESKTOP_REQUIRED",
      message: "Open the FrameIQ desktop app to access local video files.",
    });
  return invoke<T>(name, args);
}
export const api = {
  renameProject: (id: string, name: string) =>
    call<void>("rename_project", { id, name }),
  renameSource: (id: string, name: string) =>
    call<void>("rename_source", { id, name }),
  markProjectOpened: (id: string) => call<void>("mark_project_opened", { id }),
  projectActivity: () => call<Record<string, string>>("project_activity"),
  modelCatalog: () => call<ModelManifestV1[]>("model_catalog"),
  prepareCloudReview: (request: CloudReviewRequestV1) =>
    call<JobEvent>("prepare_cloud_review", { request }),
  cloudReview: (id: string) => call<CloudReviewV1>("cloud_review", { id }),
  discardCloudReview: (id: string) =>
    call<void>("discard_cloud_review", { id }),
  analyzeCloudReview: (id: string, consent: boolean) =>
    call<JobEvent>("analyze_cloud_review", { id, consent }),
  cloudResult: (id: string) =>
    call<CloudAnalysisResultV1 | null>("cloud_result", { id }),
  cloudKeyStatus: (provider: CloudProviderV1) =>
    call<CloudCredentialStatusV1>("cloud_key_status", { provider }),
  saveCloudKey: (provider: CloudProviderV1, apiKey: string) =>
    call<void>("save_cloud_key", { provider, apiKey }),
  removeCloudKey: (provider: CloudProviderV1) =>
    call<void>("remove_cloud_key", { provider }),
  testCloudConnection: (provider: CloudProviderV1) =>
    call<CloudConnectionV1>("test_cloud_connection", { provider }),
  readModelManifest: (path: string) =>
    call<ModelManifestV1>("read_model_manifest", { path }),
  executePreset: (request: PresetExecutionV1) =>
    call<JobEvent>("execute_preset", { request }),
  selectPresetFrames: (request: PresetExecutionV1) =>
    call<JobEvent>("select_preset_frames", { request }),
  presetResult: (job: string) =>
    call<PresetResultV1 | null>("preset_result", { job }),
  presets: () => call<PresetV1[]>("presets"),
  savePreset: (preset: PresetV1) => call<PresetV1>("save_preset", { preset }),
  regions: (source: string) => call<GuidedRegionV1[]>("regions", { source }),
  removeRegion: (id: string) => call<void>("remove_region", { id }),
  measureRegion: (
    source: string,
    ordinal: string,
    preset: string,
    rectangle: RegionRectV1,
  ) => call<JobEvent>("measure_region", { source, ordinal, preset, rectangle }),
  detectFaces: (source: string, ordinal: string, model: string) =>
    call<JobEvent>("detect_faces", { source, ordinal, model }),
  faceFeature: (source: string, ordinal: string, model: string) =>
    call<FaceFrameV1 | null>("face_feature", { source, ordinal, model }),
  chooseSubject: (person: string, index: number) =>
    call<SubjectChoiceV1>("choose_subject", { person, index }),
  subjectChoice: (source: string, ordinal: string) =>
    call<SubjectChoiceV1 | null>("subject_choice", { source, ordinal }),
  clearSubject: (source: string, ordinal: string) =>
    call<void>("clear_subject", { source, ordinal }),
  detectPersons: (source: string, ordinal: string, model: string) =>
    call<JobEvent>("detect_persons", { source, ordinal, model }),
  personFeature: (source: string, ordinal: string, model: string) =>
    call<PersonFrameV1 | null>("person_feature", { source, ordinal, model }),
  estimatePose: (person: string, index: number, model: string) =>
    call<JobEvent>("estimate_pose", { person, index, model }),
  poseFeature: (person: string, index: number, model: string) =>
    call<PoseFrameV1 | null>("pose_feature", { person, index, model }),
  models: () => call<ModelInstallationV1[]>("models"),
  hardware: () => call<HardwareProfileV1>("hardware"),
  benchmarks: () => call<ModelBenchmarkV1[]>("model_benchmarks"),
  importModel: (path: string, manifest: ModelManifestV1) =>
    call<JobEvent>("import_model", { path, manifest }),
  downloadModel: (manifest: ModelManifestV1) =>
    call<JobEvent>("download_model", { manifest, reviewed: true }),
  benchmarkModel: (id: string) => call<JobEvent>("benchmark_model", { id }),
  removeModel: (id: string) => call<void>("remove_model", { id }),
  saveRuntimeToken: (runtime: string, token: string) =>
    call<void>("save_runtime_token", { runtime, token }),
  discoverRuntime: (runtime: string, port: number) =>
    call<LocalRuntimeReportV1>("discover_runtime", { runtime, port }),
  probeRuntime: (runtime: string, port: number, model: string) =>
    call<number>("probe_runtime", { runtime, port, model }),
  projects: () => (desktop ? call<Project[]>("projects") : Promise.resolve([])),
  createProject: (name: string) => call<Project>("create_project", { name }),
  sources: (project: string) => call<SourceSummary[]>("sources", { project }),
  frames: (source: string, start: string, limit = 128) =>
    call<IndexedFrame[]>("frames", { source, start, limit }),
  resolve: (source: string, time: number) =>
    call<IndexedFrame>("resolve_frame", { source, time }),
  preview: (source: string, ordinal: string) =>
    call<FrameView>("preview_frame", { source, ordinal }),
  thumbnail: (source: string, ordinal: string) =>
    call<FrameView>("thumbnail", { source, ordinal }),
  selections: (project: string) => call<Selection[]>("selections", { project }),
  select: (source: string, ordinal: string, selected: boolean) =>
    call<void>("select_frame", { source, ordinal, selected }),
  selectAll: (source: string) => call<void>("select_all_frames", { source }),
  clearSelections: (project: string) =>
    call<void>("clear_selections", { project }),
  addSelections: (project: string, frames: Selection[]) =>
    call<void>("add_selections", { project, frames }),
  jobs: () => (desktop ? call<JobEvent[]>("list_jobs") : Promise.resolve([])),
  cancel: (id: string) => call<void>("cancel_job", { id }),
  import: (project: string, path: string) =>
    call<JobEvent>("import_video", { project, path, stream: null }),
  export: (project: string, destination: string, options: ExportOptions) =>
    call<JobEvent>("export_frames", { project, destination, options }),
  analyze: (source: string) => call<JobEvent>("analyze_source", { source }),
  recommendations: (source: string, config: RankingConfigV1) =>
    call<RecommendationSetV1 | null>("recommendations", { source, config }),
  relocate: (source: string, path: string) =>
    call<JobEvent>("relocate_source", { source, path }),
  exportPlans: (project: string) =>
    call<ExportPlanV1[]>("export_plans", { project }),
  exportAttemptSummary: (job: string) =>
    call<ExportAttemptSummaryV1>("export_attempt_summary", { job }),
  resumeExport: (plan: string) => call<JobEvent>("resume_export", { plan }),
  dismissExport: (plan: string) => call<void>("dismiss_export", { plan }),
  onJob: (fn: (event: JobEvent) => void) =>
    desktop
      ? listen<JobEvent>("frameiq://job", (e) => fn(e.payload))
      : Promise.resolve(() => {}),
  image: (path: string) => convertFileSrc(path),
};
export function message(error: unknown): string {
  if (typeof error === "object" && error && "message" in error)
    return String(error.message);
  return String(error);
}
export function timestamp(seconds: number): string {
  const n = Math.max(0, Math.round(seconds * 1000));
  const h = Math.floor(n / 3600000),
    m = Math.floor(n / 60000) % 60,
    s = Math.floor(n / 1000) % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(n % 1000).padStart(3, "0")}`;
}
export function parseTimestamp(value: string): number | null {
  if (/^\d+(\.\d+)?$/.test(value)) return Number(value);
  const m = /^(\d+):([0-5]\d):([0-5]\d)(?:\.(\d{1,3}))?$/.exec(value);
  if (!m) return null;
  return (
    Number(m[1]) * 3600 +
    Number(m[2]) * 60 +
    Number(m[3]) +
    Number((m[4] ?? "").padEnd(3, "0")) / 1000
  );
}
