import { useEffect, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, desktop, message } from "../client";
import type { CloudProviderV1 } from "../contracts";

function Provider({
  provider,
  name,
}: {
  provider: CloudProviderV1;
  name: string;
}) {
  const [key, setKey] = useState("");
  const [saved, setSaved] = useState(false);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    if (desktop)
      void api
        .cloudKeyStatus(provider)
        .then((s) => {
          if (live) {
            setSaved(s.saved);
            setReady(true);
          }
        })
        .catch((e) => {
          if (live) setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [provider]);
  async function action(run: () => Promise<void>) {
    setBusy(true);
    setError("");
    setStatus("");
    try {
      await run();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section
      className="cloud-provider"
      aria-labelledby={`${provider}-heading`}
      aria-busy={busy}
    >
      <h3 id={`${provider}-heading`}>{name}</h3>
      <p>
        {ready
          ? saved
            ? "Key saved in OS credential storage."
            : "No key saved."
          : desktop
            ? error
              ? "Key status unavailable."
              : "Checking key storage…"
            : "No key saved."}
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!key.trim() || busy || !desktop) return;
          void action(async () => {
            await api.saveCloudKey(provider, key.trim());
            setKey("");
            setSaved(true);
            setReady(true);
            setStatus("Key saved.");
          });
        }}
      >
        <label htmlFor={`${provider}-key`}>API key</label>
        <input
          id={`${provider}-key`}
          type="password"
          autoComplete="off"
          autoCapitalize="none"
          spellCheck={false}
          value={key}
          maxLength={8192}
          placeholder={saved ? "Enter replacement key" : "Enter API key"}
          disabled={busy || !desktop}
          onChange={(e) => setKey(e.target.value)}
        />
        <div className="model-actions">
          <Button type="submit" disabled={busy || !desktop || !key.trim()}>
            Save key
          </Button>
          <Button
            type="button"
            disabled={busy || !desktop || !saved || !ready}
            onClick={() =>
              void action(async () => {
                setStatus("Testing connection…");
                const report = await api.testCloudConnection(provider);
                setStatus(
                  `Connected. ${report.modelCount} models listed${report.moreAvailable ? " on the first page" : ""}.`,
                );
              })
            }
          >
            Test connection
          </Button>
          <Button
            type="button"
            disabled={busy || !desktop || !saved || !ready}
            onClick={() =>
              void action(async () => {
                await api.removeCloudKey(provider);
                setKey("");
                setSaved(false);
                setStatus("Key removed.");
              })
            }
          >
            Remove key
          </Button>
        </div>
      </form>
      <p role="status">{status}</p>
      {error && (
        <p className="cloud-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
export function CloudProviders() {
  return (
    <section className="cloud-providers" aria-labelledby="cloud-heading">
      <h2 id="cloud-heading">Cloud AI</h2>
      <p>
        Connection tests send your key to the selected provider to list models.
        No images or video are sent.
      </p>
      <Provider provider="openai" name="OpenAI" />
      <Provider provider="gemini" name="Gemini" />
      <p>
        Saving a key does not send images. Review cloud requests in Studio. Keys
        are kept out of projects and exports.
      </p>
      {!desktop && <p>Open the desktop app to manage keys.</p>}
    </section>
  );
}
