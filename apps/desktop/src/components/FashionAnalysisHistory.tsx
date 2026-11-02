import { useEffect, useState } from "react";
import { fashionApi } from "../fashionClient";
import { message } from "../client";
type Run = {
  jobId: string;
  review: {
    request: { model: string; runtime: string | null; task: string };
    images: { sha256: string }[];
  };
  outcome: { attributes: { name: string; value: string; origin: string }[] };
};
export function FashionAnalysisHistory({
  project,
  revision,
}: {
  project: string;
  revision: unknown;
}) {
  const [runs, setRuns] = useState<Run[]>([]),
    [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    void fashionApi
      .history(project)
      .then((r) => {
        if (live) setRuns(r as Run[]);
      })
      .catch((e) => {
        if (live) setError(message(e));
      });
    return () => {
      live = false;
    };
  }, [project, revision]);
  return (
    <details>
      <summary>Analysis history and comparison</summary>
      <p>
        Compare observed details against the source. Matching image hashes mean
        identical reviewed inputs; repeated inputs add no independent identity
        evidence. Model agreement does not prove accuracy.
      </p>
      {error && <p role="alert">{error}</p>}
      {!runs.length && <p>No completed analyses yet.</p>}
      {runs.map((r) => (
        <article key={r.jobId}>
          <h3>
            {r.review.request.runtime || "ChatGPT"} · {r.review.request.model}
          </h3>
          <p>
            {r.review.request.task} · images{" "}
            {r.review.images.map((i) => i.sha256.slice(0, 12)).join(", ")}
          </p>
          {r.outcome.attributes.length > 0 && (
            <dl>
              {r.outcome.attributes.map((a, i) => (
                <div key={i}>
                  <dt>
                    {a.name} ({a.origin})
                  </dt>
                  <dd>{a.value}</dd>
                </div>
              ))}
            </dl>
          )}
        </article>
      ))}
    </details>
  );
}
