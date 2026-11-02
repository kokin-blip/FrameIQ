import { useEffect, useState } from "react";
import { Button } from "@frameiq/ui";
import { fashionApi } from "../fashionClient";
import { message } from "../client";
export function ShoppingProviders() {
  const [saved, setSaved] = useState(false),
    [id, setId] = useState(""),
    [secret, setSecret] = useState(""),
    [approved, setApproved] = useState(false),
    [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    void fashionApi
      .ebayStatus()
      .then((s) => {
        if (live) setSaved(s);
      })
      .catch((e) => {
        if (live) setError(message(e));
      });
    return () => {
      live = false;
    };
  }, []);
  return (
    <section className="settings-section">
      <div>
        <h2>Optional shopping adapter</h2>
        <p>
          ChatGPT remains the primary research provider. An authorized eBay
          application can search the same reviewed item crops. No paid-service
          fallback is enabled. Reviewed requests are capped locally at 100 per
          provider/account per UTC day; provider limits still apply.
        </p>
        <p>
          eBay credentials:{" "}
          {saved ? "saved in OS credential storage" : "not configured"}
        </p>
        {error && <p role="alert">{error}</p>}
        <label>
          eBay client ID
          <input
            value={id}
            onChange={(e) => setId(e.target.value)}
            autoComplete="off"
          />
        </label>
        <label>
          eBay client secret
          <input
            type="password"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            autoComplete="off"
          />
        </label>
        <label>
          <input
            type="checkbox"
            checked={approved}
            onChange={(e) => setApproved(e.target.checked)}
          />
          My application has approved production access and permits this use.
        </label>
        <div className="fashion-actions">
          <Button
            disabled={!approved || !id || !secret}
            onClick={() =>
              void fashionApi
                .saveEbay(id, secret, approved)
                .then(() => {
                  setId("");
                  setSecret("");
                  setSaved(true);
                  setError("");
                })
                .catch((e) => setError(message(e)))
            }
          >
            Save eBay credentials
          </Button>
          <Button
            disabled={!saved}
            onClick={() =>
              void fashionApi
                .removeEbay()
                .then(() => {
                  setSaved(false);
                  setError("");
                })
                .catch((e) => setError(message(e)))
            }
          >
            Remove eBay credentials
          </Button>
        </div>
      </div>
    </section>
  );
}
