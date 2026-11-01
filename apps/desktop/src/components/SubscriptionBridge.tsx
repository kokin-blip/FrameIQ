import { useEffect, useState } from "react";
import { Button } from "@frameiq/ui";
import { api, desktop, message } from "../client";
import type { SubscriptionStatusV1 } from "../contracts";

export function SubscriptionBridge() {
  const [status, setStatus] = useState<SubscriptionStatusV1 | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const active = status?.accounts.find((a) => a.id === status.activeAccountId);
  useEffect(() => {
    if (!desktop) return;
    let live = true;
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const next = await api.subscriptionStatus();
        if (live) {
          setStatus(next);
          if (next.signingIn) timer = setTimeout(() => void refresh(), 1000);
        }
      } catch (e) {
        if (live) setError(message(e));
      }
    }
    void refresh();
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [busy]);
  async function action(run: () => Promise<void>) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await run();
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(false);
    }
  }
  const pending = !!status?.signingIn;
  return (
    <section
      className="cloud-provider subscription-bridge"
      aria-labelledby="subscription-heading"
      aria-busy={busy || pending}
    >
      <h3 id="subscription-heading">ChatGPT subscription</h3>
      <p>
        Bring your own ChatGPT subscription. Sign in once to describe frames and
        regions, inspect visible details, and rank chosen frames for any preset
        without installing separate models for those requests.
      </p>
      <p>
        Exact extraction, person tracking, pose landmarks, and local quality
        measurements continue to use the existing local tools.
      </p>
      {status && status.accounts.length > 0 && (
        <label className="connector-field">
          ChatGPT account
          <select
            value={status.activeAccountId ?? ""}
            disabled={busy || pending}
            onChange={(e) =>
              void action(() => api.selectSubscriptionAccount(e.target.value))
            }
          >
            {!status.activeAccountId && (
              <option value="">Choose an account</option>
            )}
            {status.accounts.map((a) => (
              <option value={a.id} key={a.id}>
                {a.label}
                {a.connected ? "" : " · signed out"}
              </option>
            ))}
          </select>
        </label>
      )}
      <p role="status">
        {!desktop
          ? "Open the desktop app to connect ChatGPT."
          : !status
            ? "Checking subscription connection…"
            : pending
              ? "Finish signing in with ChatGPT in your browser…"
              : active?.connected
                ? `Connected: ${active.label}`
                : "No ChatGPT subscription connected."}
      </p>
      <div className="model-actions">
        <Button
          variant="primary"
          data-oai-siwc-button
          disabled={!desktop || busy || pending || !status}
          onClick={() =>
            void action(() =>
              api.startSubscriptionLogin(status?.activeAccountId ?? null),
            )
          }
        >
          Continue with ChatGPT
        </Button>
        {pending ? (
          <Button
            disabled={busy}
            onClick={() => void action(() => api.cancelSubscriptionLogin())}
          >
            Cancel sign-in
          </Button>
        ) : (
          <>
            {!!status?.accounts.length && (
              <Button
                disabled={busy}
                onClick={() =>
                  void action(() => api.startSubscriptionLogin(null))
                }
              >
                Add ChatGPT account
              </Button>
            )}
            <Button
              disabled={!active?.connected || busy || !desktop}
              onClick={() =>
                void action(async () => {
                  const report = await api.testCloudConnection("chatgpt");
                  setNotice(
                    `Connected. ${report.modelCount} subscription models available.`,
                  );
                })
              }
            >
              Test subscription
            </Button>
            <Button
              disabled={!active?.connected || busy || !desktop}
              onClick={() => void action(() => api.disconnectSubscription())}
            >
              Sign out
            </Button>
          </>
        )}
      </div>
      <p>
        Plan availability and usage limits apply. Requests use your selected
        ChatGPT account; there is no automatic API-key fallback.{" "}
        <a
          href="https://chatgpt.com/settings/usage"
          target="_blank"
          rel="noreferrer"
        >
          Manage ChatGPT usage and access
        </a>
        .
      </p>
      <p>
        OAuth credentials stay in OS credential storage. Images are sent only
        after you review and approve them in Studio.
      </p>
      {notice && <p role="status">{notice}</p>}
      {(error || status?.error) && (
        <p className="cloud-error" role="alert">
          {error || status?.error?.message}
        </p>
      )}
      {error && desktop && (
        <Button
          disabled={busy}
          onClick={() =>
            void action(async () => setStatus(await api.subscriptionStatus()))
          }
        >
          Check connection again
        </Button>
      )}
    </section>
  );
}
