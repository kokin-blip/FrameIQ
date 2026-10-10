import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, desktop, message } from "../client";
import type {
  HardwareProfileV1,
  ModelInstallationV1,
  ModelBenchmarkV1,
  ModelManifestV1,
  LocalRuntimeReportV1,
} from "../contracts";

const gib = (bytes: string) => `${(Number(bytes) / 1073741824).toFixed(1)} GiB`;
export function ModelManager() {
  const [hardware, setHardware] = useState<HardwareProfileV1>();
  const [models, setModels] = useState<ModelInstallationV1[]>([]);
  const [benchmarks, setBenchmarks] = useState<ModelBenchmarkV1[]>([]);
  const [manifest, setManifest] = useState<ModelManifestV1>();
  const [reviewed, setReviewed] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [token,setToken]=useState("");
  const [runtime, setRuntime] = useState("ollama");
  const [port, setPort] = useState(11434);
  const [report, setReport] = useState<LocalRuntimeReportV1>();
  const [probe, setProbe] = useState("");
  async function refresh() {
    const [h, m, b] = await Promise.all([
      api.hardware(),
      api.models(),
      api.benchmarks(),
    ]);
    setHardware(h);
    setModels(m.filter((item) => item.status !== "removed"));
    setBenchmarks(b);
  }
  useEffect(() => {
    if (!desktop) return;
    void refresh().catch((e) => setError(message(e)));
    const listener = api.onJob((e) => {
      if (
        e.stage.startsWith("model") &&
        ["succeeded", "failed", "cancelled"].includes(e.status)
      )
        void refresh().catch((e) => setError(message(e)));
    });
    return () => {
      void listener.then((stop) => stop());
    };
  }, []);
  async function action(fn: () => Promise<unknown>) {
    setError("");
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  async function chooseManifest() {
    const path = await open({
      multiple: false,
      filters: [{ name: "Model manifest", extensions: ["json"] }],
    });
    if (typeof path === "string") {
      setManifest(await api.readModelManifest(path));
      setReviewed(false);
    }
  }
  return (
    <div className="model-manager">
      <h2>Local models</h2>
      <p>
        Models run in a separate worker. Compatibility checks do not establish
        detection accuracy.
      </p>
      {hardware && (
        <dl className="shortcut-list">
          <dt>Computer</dt>
          <dd>
            {hardware.architecture} · {hardware.logicalCores} cores
          </dd>
          <dt>Memory / available</dt>
          <dd>
            {gib(hardware.totalMemoryBytes)} /{" "}
            {gib(hardware.availableMemoryBytes)}
          </dd>
          <dt>Managed AI budget</dt>
          <dd>
            {gib(hardware.aiBudgetBytes)} · {hardware.workerThreads} shared CPU
            slots · one AI operation
          </dd>
        </dl>
      )}
      {error && (
        <p role="alert" className="error-banner">
          {error}
        </p>
      )}
      <button
        disabled={!desktop || busy}
        onClick={() => void action(chooseManifest)}
      >
        Open model manifest
      </button>
      {manifest && (
        <section className="model-review">
          <h3>{manifest.name}</h3>
          <p>
            {manifest.task} · {(Number(manifest.byteSize) / 1048576).toFixed(1)}{" "}
            MiB · {manifest.license}
          </p>
          <p className="wrap-text">Source: {manifest.sourceUrl}</p>
          <p className="wrap-text">License: {manifest.licenseUrl}</p>
          <p className="wrap-text">SHA-256: {manifest.sha256}</p>
          <label>
            <input
              type="checkbox"
              checked={reviewed}
              onChange={(e) => setReviewed(e.target.checked)}
            />{" "}
            I reviewed the source, size, license, and checksum.
          </label>
          <div className="model-actions">
            <button
              disabled={busy || !reviewed}
              onClick={() =>
                void action(async () => {
                  await api.downloadModel(manifest);
                })
              }
            >
              Download and validate
            </button>
            <button
              disabled={busy}
              onClick={() =>
                void action(async () => {
                  const path = await open({
                    multiple: false,
                    filters: [{ name: "ONNX model", extensions: ["onnx"] }],
                  });
                  if (typeof path === "string")
                    await api.importModel(path, manifest);
                })
              }
            >
              Import matching ONNX file
            </button>
          </div>
          <p>
            Progress and cancellation are in Activity. A previous installation
            remains usable until validation succeeds.
          </p>
        </section>
      )}
      {models.length === 0 ? (
        <p>No models installed.</p>
      ) : (
        models.map((m) => {
          const b = benchmarks.find((b) => b.modelId === m.id);
          return (
            <section className="model-review" key={m.id}>
              <h3>{m.manifest.name}</h3>
              <p>
                {m.manifest.task} ·{" "}
                {m.status === "ready" ? "Compatibility checked" : m.status}
              </p>
              {b && (
                <p>
                  CPU · warm {b.warmLatencyMs.toFixed(0)} ms · cold load{" "}
                  {b.coldLoadMs.toFixed(0)} ms · observed memory{" "}
                  {gib(b.peakObservedMemoryBytes)}
                </p>
              )}
              <div className="model-actions">
                <button
                  disabled={busy}
                  onClick={() =>
                    void action(async () => {
                      await api.benchmarkModel(m.id);
                    })
                  }
                >
                  Benchmark
                </button>
                <button
                  disabled={busy}
                  onClick={() =>
                    void action(async () => {
                      await api.removeModel(m.id);
                      await refresh();
                    })
                  }
                >
                  Remove
                </button>
              </div>
            </section>
          );
        })
      )}
      <h2>Local runtime connections</h2>
      <p>
        Connect only when requested. Requests use 127.0.0.1. The compatibility
        test sends a synthetic pixel, never a project image. FrameIQ does not
        control these servers' memory.
      </p>
      <div className="model-actions">
        <label>
          Runtime
          <select
            value={runtime}
            onChange={(e) => {
              setRuntime(e.target.value);
              setPort(e.target.value === "ollama" ? 11434 : 1234);
              setReport(undefined);
              setProbe("");
            }}
          >
            <option value="ollama">Ollama</option>
            <option value="lm_studio">LM Studio</option>
          </select>
        </label>
        <label>
          Port
          <input
            type="number"
            min="1"
            max="65535"
            value={port}
            onChange={(e) => {
              setPort(Number(e.target.value));
              setReport(undefined);
            }}
          />
        </label>
        <button
          disabled={busy || !desktop}
          onClick={() =>
            void action(async () =>
              setReport(await api.discoverRuntime(runtime, port)),
            )
          }
        >
          Connect and list models
        </button>
      </div>
      <label>Optional runtime token<input type="password" autoComplete="off" value={token} onChange={e=>setToken(e.target.value)} /></label>
      <button disabled={busy || !desktop} onClick={()=>void action(async()=>{await api.saveRuntimeToken(runtime,token);setToken("");})}>Save token in OS credential storage</button>
      <p>An empty token removes the saved credential. Tokens are never stored in projects or exported files.</p>
      {report &&
        report.models.map((m) => (
          <div className="model-review" key={m.id}>
            <span>
              {m.id} · {m.local ? "local" : "remote"} ·{" "}
              {m.vision ? "image input" : "text only"}
            </span>
            <button
              disabled={busy || !m.local || !m.vision}
              onClick={() =>
                void action(async () =>
                  setProbe(
                    `${m.id}: synthetic image response in ${(await api.probeRuntime(runtime, port, m.id)).toFixed(0)} ms`,
                  ),
                )
              }
            >
              Test image input
            </button>
          </div>
        ))}
      <p role="status">{probe}</p>
    </div>
  );
}
