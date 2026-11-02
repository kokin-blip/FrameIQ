import { useFashionPreferences, useFashionTab } from "../fashionPreferences";
import { FashionAnalysisHistory } from "./FashionAnalysisHistory";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@frameiq/ui";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ImagePlus,
  Scan,
  Save,
  Undo2,
  Trash2,
  ExternalLink,
  FolderOpen,
} from "lucide-react";
import { api, desktop, message } from "../client";
import { fashionApi } from "../fashionClient";
import { useJobs, useSettingsTab } from "../store";
import { FashionCanvas } from "./FashionCanvas";
import type { Workspace } from "../workspace/useWorkspace";
import type {
  AssetRefV1,
  FashionItemV1,
  FashionReviewV1,
  FashionTaskV1,
  FeatureCapabilityV1,
  GarmentGroupV1,
  JobEvent,
  MediaRefV1,
  PhotoSourceV1,
  ProductCandidateV1,
  SavedLookV1,
  FashionExportV1,
} from "../contracts";
const categories = [
  "dress",
  "top",
  "shirt",
  "outerwear",
  "trousers",
  "skirt",
  "footwear",
  "bag",
  "jewelry",
  "sunglasses",
  "hat",
  "hair_accessory",
  "belt",
  "hosiery",
  "accessory",
  "hair",
  "makeup",
  "unknown",
];
const same = (a: unknown, b: unknown) =>
  JSON.stringify(a) === JSON.stringify(b);
const freshLook = (projectId: string): SavedLookV1 => ({
  id: crypto.randomUUID(),
  projectId,
  name: "New look",
  notes: "",
  items: [],
  revision: 0,
});
export function FashionStudio({ w }: { w: Workspace }) {
  const project = w.project?.id ?? "";
  const { tab, setTab } = useFashionTab();
  const tasks = useFashionPreferences((s) => s.tasks);
  const projectRef = useRef(project);
  projectRef.current = project;
  const pendingSelection = useRef("");
  const [photos, setPhotos] = useState<PhotoSourceV1[]>([]),
    [items, setItems] = useState<FashionItemV1[]>([]),
    [groups, setGroups] = useState<GarmentGroupV1[]>([]),
    [looks, setLooks] = useState<SavedLookV1[]>([]),
    [exports, setExports] = useState<FashionExportV1[]>([]),
    [capabilities, setCapabilities] = useState<FeatureCapabilityV1[]>([]);
  const [media, setMedia] = useState<MediaRefV1 | null>(null),
    [asset, setAsset] = useState<AssetRefV1 | null>(null),
    [selected, setSelected] = useState(""),
    [draft, setDraft] = useState<FashionItemV1 | null>(null),
    [attributes, setAttributes] = useState("");
  const [mode, setMode] = useState("inspect"),
    [showMask, setShowMask] = useState(false),
    [error, setError] = useState(""),
    [notice, setNotice] = useState(""),
    [busy, setBusy] = useState(false);
  const [chosen, setChosen] = useState<string[]>([]),
    [groupId, setGroupId] = useState(""),
    [groupName, setGroupName] = useState("Garment"),
    [products, setProducts] = useState<ProductCandidateV1[]>([]),
    [query, setQuery] = useState(""),
    [shoppingProvider, setShoppingProvider] = useState("chatgpt");
  const [model, setModel] = useState(""),
    [models, setModels] = useState<string[]>([]),
    [runtime, setRuntime] = useState(""),
    [port, setPort] = useState(11434),
    [task, setTask] = useState<FashionTaskV1>("detect");
  const [review, setReview] = useState<FashionReviewV1 | null>(null),
    [loaded, setLoaded] = useState<string[]>([]),
    [consent, setConsent] = useState(false),
    [account, setAccount] = useState<string | null>(null),
    [jobId, setJobId] = useState("");
  const [matchFilter, setMatchFilter] = useState("all"),
    [brand, setBrand] = useState(""),
    [retailer, setRetailer] = useState(""),
    [categoryFilter, setCategoryFilter] = useState(""),
    [minPrice, setMinPrice] = useState(""),
    [maxPrice, setMaxPrice] = useState("");
  const [identifier, setIdentifier] = useState(""),
    [look, setLook] = useState<SavedLookV1>(() => freshLook(project)),
    [format, setFormat] = useState("html");
  const jobs = useJobs((s) => s.jobs),
    setJob = useJobs((s) => s.setJob);
  const currentJob = jobs[jobId];
  const running =
    busy ||
    (!!currentJob &&
      ["queued", "running", "cancelling"].includes(currentJob.status));
  const group = groups.find((g) => g.id === groupId),
    current = items.find((i) => i.id === selected);
  const visible = items.filter((i) => media && same(i.media, media));
  const preparedContext = useRef("");
  useEffect(() => {
    const p = tasks[task];
    setRuntime(p.runtime);
    setPort(p.port);
    setModel(p.model);
    setModels(p.model ? [p.model] : []);
  }, [task, tasks]);
  const context = JSON.stringify([
    project,
    media,
    selected,
    current?.revision,
    groupId,
    group?.revision,
    model,
    runtime,
    port,
    task,
    account,
    shoppingProvider,
  ]);
  const [confirmationId, setConfirmationId] = useState("");
  const [lastDeleted, setLastDeleted] = useState("");
  const conflicts = Object.entries(
    items
      .filter((i) => group?.itemIds.includes(i.id))
      .flatMap((i) => i.attributes)
      .reduce<Record<string, string[]>>(
        (m, a) => ({
          ...m,
          [a.name]: [...new Set([...(m[a.name] ?? []), a.value])],
        }),
        {},
      ),
  ).filter(([, values]) => values.length > 1);
  const generation = useRef(0);
  const refresh = useCallback(async () => {
    if (!project) return;
    const [p, i, g, l, c, e] = await Promise.all([
      fashionApi.photos(project),
      fashionApi.items(project),
      fashionApi.groups(project),
      fashionApi.looks(project),
      fashionApi.capabilities(),
      fashionApi.exports(project),
    ]);
    if (projectRef.current !== project) return;
    setPhotos(p);
    setItems(i);
    setGroups(g);
    setLooks(l);
    setCapabilities(c);
    setExports(e);
  }, [project]);
  useEffect(() => {
    let live = true;
    setMedia(null);
    setAsset(null);
    setSelected("");
    setGroupId("");
    setProducts([]);
    setChosen([]);
    setLastDeleted("");
    setLook(freshLook(project));
    if (project)
      void refresh().catch((e) => {
        if (live) setError(message(e));
      });
    return () => {
      live = false;
    };
  }, [project, refresh]);
  useEffect(() => {
    let live = true;
    const update = () =>
      void api
        .subscriptionStatus()
        .then((s) => {
          if (live) setAccount(s.activeAccountId);
        })
        .catch(() => {});
    update();
    window.addEventListener("focus", update);
    return () => {
      live = false;
      window.removeEventListener("focus", update);
    };
  }, []);
  const frame =
    w.image && w.sourceId
      ? {
          sourceId: w.sourceId,
          frameRef: w.image.frameRef,
          timeSeconds: w.image.timeSeconds,
        }
      : null;
  useEffect(() => {
    if (media) return;
    if (frame) setMedia({ kind: "frame", selection: frame });
    else if (photos[0])
      setMedia({ kind: "photo", revision: photos[0].revision });
  }, [photos, !!frame, media]);
  useEffect(() => {
    const n = ++generation.current;
    setAsset(null);
    setSelected("");
    setDraft(null);
    setMode("inspect");
    if (media && project)
      void fashionApi
        .media(project, media)
        .then((a) => {
          if (generation.current === n) {
            setAsset(a);
            if (pendingSelection.current) {
              setSelected(pendingSelection.current);
              pendingSelection.current = "";
            }
          }
        })
        .catch((e) => {
          if (generation.current === n) setError(message(e));
        });
  }, [JSON.stringify(media), project]);
  useEffect(() => {
    if (current) {
      setDraft({
        ...current,
        rectangle: { ...current.rectangle },
        polygon: [...current.polygon],
      });
      setAttributes(
        current.attributes.map((a) => `${a.name}: ${a.value}`).join("\n"),
      );
    }
  }, [current?.id, current?.revision]);
  useEffect(() => {
    setIdentifier("");
    setConfirmationId("");
  }, [groupId]);
  useEffect(() => {
    setReview(null);
    setConsent(false);
    setLoaded([]);
  }, [context]);
  useEffect(
    () => () => {
      if (review) void fashionApi.discard(review.id).catch(() => {});
    },
    [review?.id],
  );
  useEffect(() => {
    let live = true;
    setProducts([]);
    if (groupId)
      void fashionApi
        .candidates(groupId)
        .then((p) => {
          if (live) setProducts(p);
        })
        .catch((e) => {
          if (live) setError(message(e));
        });
    return () => {
      live = false;
    };
  }, [groupId, group?.revision, jobs]);
  useEffect(() => {
    if (
      !currentJob ||
      !["succeeded", "failed", "cancelled"].includes(currentJob.status)
    )
      return;
    let live = true;
    if (currentJob.error) setError(currentJob.error.message);
    void refresh().catch((e) => setError(message(e)));
    if (
      currentJob.stage === "fashion_preparing" &&
      currentJob.status === "succeeded"
    ) {
      if (preparedContext.current === context)
        void fashionApi
          .review(currentJob.jobId)
          .then((r) => {
            if (live) setReview(r);
          })
          .catch((e) => setError(message(e)));
      else void fashionApi.discard(currentJob.jobId).catch(() => {});
    } else if (currentJob.status === "succeeded") {
      setNotice(
        currentJob.stage === "fashion_analysis" &&
          currentJob.resultIds.length === 0
          ? "No supported items or product matches were found."
          : "Saved.",
      );
      if (currentJob.stage === "photo_import")
        void fashionApi.photos(project).then((p) => {
          const added = p.find((p) =>
            currentJob.resultIds.includes(p.revision),
          );
          if (added) setMedia({ kind: "photo", revision: added.revision });
        });
    }
    return () => {
      live = false;
    };
  }, [currentJob?.status, currentJob?.jobId]);
  async function perform<T>(fn: () => Promise<T>) {
    setError("");
    setBusy(true);
    try {
      return await fn();
    } catch (e) {
      setError(message(e));
      return undefined;
    } finally {
      setBusy(false);
    }
  }
  function track(j: JobEvent) {
    setJob(j);
    setJobId(j.jobId);
    setNotice("");
    return j;
  }
  async function importPhoto() {
    await perform(async () => {
      const p = await open({
        multiple: false,
        filters: [
          { name: "Photos", extensions: ["jpg", "jpeg", "png", "webp"] },
        ],
      });
      if (typeof p === "string")
        track(await fashionApi.importPhoto(project, p));
    });
  }
  function newItem() {
    if (!media) return;
    setSelected("");
    setAttributes("");
    setDraft({
      id: "",
      projectId: project,
      media,
      revision: 0,
      category: "unknown",
      rectangle: { x: 0, y: 0, width: 1, height: 1 },
      polygon: [],
      mask: null,
      crop: null,
      attributes: [],
      suppressedAttributes: [],
      detectionScore: null,
      provenance: "user_drawn",
      modelHash: null,
      corrected: true,
      deleted: false,
    });
    setMode("box");
  }
  async function saveItem() {
    if (!draft) return;
    await perform(async () => {
      let rectangle = draft.rectangle;
      if (draft.polygon.length >= 3) {
        const xs = draft.polygon.map((p) => p[0]),
          ys = draft.polygon.map((p) => p[1]);
        rectangle = {
          x: Math.min(...xs),
          y: Math.min(...ys),
          width: Math.max(...xs) - Math.min(...xs),
          height: Math.max(...ys) - Math.min(...ys),
        };
      }
      const attrs = attributes
        .split("\n")
        .filter((l) => l.trim())
        .map((l) => {
          const n = l.indexOf(":");
          if (n < 1)
            throw new Error("Use one attribute per line: name: value.");
          return {
            name: l.slice(0, n).trim(),
            value: l.slice(n + 1).trim(),
            origin: "user",
            confidence: null,
            evidenceIds: [],
          };
        });
      track(
        await fashionApi.saveItem({ ...draft, rectangle, attributes: attrs }),
      );
      setMode("inspect");
    });
  }
  async function loadModels() {
    await perform(async () => {
      if (runtime) {
        const r = await api.discoverRuntime(runtime, port);
        const ms = r.models.filter((m) => m.local && m.vision).map((m) => m.id);
        setModels(ms);
        setModel(ms[0] ?? "");
      } else {
        const c = await api.testCloudConnection("chatgpt");
        setModels(c.models);
        setModel(c.models[0] ?? "");
      }
    });
  }
  async function prepare() {
    if (!media) return;
    await perform(async () => {
      preparedContext.current = context;
      track(
        await fashionApi.prepare({
          projectId: project,
          media: task === "detect" ? [media] : [],
          itemIds: task === "attributes" && selected ? [selected] : [],
          groupId: task === "research" ? groupId : null,
          task,
          model:
            task === "research" && shoppingProvider === "ebay"
              ? "ebay-browse"
              : model,
          accountId: account,
          runtime: runtime || null,
          port: runtime ? port : null,
          shoppingProvider: task === "research" ? shoppingProvider : null,
        }),
      );
    });
  }
  async function saveGroup(split = false) {
    await perform(async () => {
      const g = await fashionApi.saveGroup({
        id: split || !group ? crypto.randomUUID() : group.id,
        projectId: project,
        name: groupName,
        revision: split || !group ? 0 : group.revision,
        itemIds: chosen,
        notes: group?.notes ?? "",
      });
      await refresh();
      setGroupId(g.id);
      setChosen(g.itemIds);
    });
  }
  function addToLook(
    gid: string,
    candidate: ProductCandidateV1 | null,
    alternative = false,
  ) {
    setLook((l) => {
      const old = l.items.find((i) => i.groupId === gid);
      const entry = {
        groupId: gid,
        candidateId: alternative
          ? (old?.candidateId ?? null)
          : (candidate?.id ?? null),
        alternativeIds:
          alternative && candidate
            ? [...new Set([...(old?.alternativeIds ?? []), candidate.id])]
            : (old?.alternativeIds ?? []),
      };
      return {
        ...l,
        items: [...l.items.filter((i) => i.groupId !== gid), entry],
      };
    });
    setNotice("Added to the look draft. Save the look to keep it.");
  }
  const filteredProducts = products.filter((c) => {
    if (matchFilter !== "all" && c.classification !== matchFilter) return false;
    if (brand && !c.product.brand?.toLowerCase().includes(brand.toLowerCase()))
      return false;
    if (
      categoryFilter &&
      !c.product.category.toLowerCase().includes(categoryFilter.toLowerCase())
    )
      return false;
    if (
      retailer &&
      !c.listings.some((l) =>
        l.retailer.toLowerCase().includes(retailer.toLowerCase()),
      )
    )
      return false;
    if (minPrice || maxPrice) {
      const prices = c.listings
        .filter((l) => l.price !== null && l.currency === "USD")
        .map((l) => Number(l.price));
      if (
        !prices.some(
          (p) =>
            (!minPrice || p >= Number(minPrice)) &&
            (!maxPrice || p <= Number(maxPrice)),
        )
      )
        return false;
    }
    return true;
  });
  if (!project)
    return (
      <section className="page">
        <h1>Fashion inspection</h1>
        <p>
          Create or open a project to inspect photos and exact video frames.
        </p>
        <Button onClick={() => w.setView("library")}>
          Open project library
        </Button>
      </section>
    );
  return (
    <section className="fashion-workspace" aria-label="Fashion inspection">
      <div className="fashion-toolbar">
        <div className="fashion-context">
          <strong>Fashion inspection</strong>
          <span>{w.project?.name}</span>
        </div>
        <Button disabled={running} onClick={() => void importPhoto()}>
          <ImagePlus size={16} />
          Import photo
        </Button>
        <label className="fashion-source">
          Source
          <select
            aria-label="Fashion source"
            value={JSON.stringify(media)}
            onChange={(e) => setMedia(JSON.parse(e.target.value))}
          >
            {!media && <option value="null">Choose media</option>}
            {frame && (
              <option
                value={JSON.stringify({ kind: "frame", selection: frame })}
              >
                Current exact video frame
              </option>
            )}
            {w.selected.map((s) => (
              <option
                key={`${s.sourceId}:${s.frameRef.presentationOrdinal}`}
                value={JSON.stringify({ kind: "frame", selection: s })}
              >
                Selected frame {s.frameRef.presentationOrdinal}
              </option>
            ))}
            {photos.map((p) => (
              <option
                key={p.revision}
                value={JSON.stringify({ kind: "photo", revision: p.revision })}
              >
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <nav aria-label="Fashion views">
          {["items", "products", "looks"].map((t) => (
            <Button key={t} aria-pressed={tab === t} onClick={() => setTab(t)}>
              {t[0].toUpperCase() + t.slice(1)}
            </Button>
          ))}
        </nav>
      </div>
      {error && (
        <div className="error-banner" role="alert">
          {error}
          <Button onClick={() => setError("")}>Dismiss</Button>
        </div>
      )}
      {notice && (
        <p className="fashion-notice" role="status">
          {notice}
        </p>
      )}
      {running && (
        <div className="fashion-progress" role="status">
          {currentJob?.stage ?? "Preparing"} ·{" "}
          {currentJob?.completedUnits ?? "0"} / {currentJob?.totalUnits ?? "…"}
          {currentJob && (
            <Button
              disabled={currentJob.status === "cancelling"}
              onClick={() => void perform(() => api.cancel(currentJob.jobId))}
            >
              Cancel task
            </Button>
          )}
        </div>
      )}
      <div className="fashion-layout">
        <div className="fashion-viewer">
          <div className="fashion-image-stage">
            {asset ? (
              <FashionCanvas
                asset={asset}
                items={visible}
                selected={selected}
                onSelect={(id) => {
                  setSelected(id);
                  setMode("inspect");
                  setTab("items");
                }}
                rectangle={
                  draft?.rectangle ?? { x: 0, y: 0, width: 1, height: 1 }
                }
                onRectangle={(r) =>
                  setDraft((d) => (d ? { ...d, rectangle: r, polygon: [] } : d))
                }
                polygon={draft?.polygon ?? []}
                onPolygon={(p) =>
                  setDraft((d) => (d ? { ...d, polygon: p } : d))
                }
                mode={mode}
                mask={showMask}
              />
            ) : (
              <div className="fashion-empty">
                <ImagePlus size={32} />
                <h2>
                  {media
                    ? "Loading source…"
                    : "Import a photo or select a video frame"}
                </h2>
                <p>
                  Keep source media, item regions and product evidence together.
                </p>
              </div>
            )}
          </div>
          <div className="fashion-source-info">
            {asset && (
              <span>
                {asset.width} × {asset.height}
              </span>
            )}
            <span>
              {media?.kind === "frame"
                ? `Exact frame ${media.selection.frameRef.presentationOrdinal}`
                : media?.kind === "photo"
                  ? "Photo revision"
                  : "No source"}
            </span>
            <Button onClick={() => w.setView("extract")}>
              Back to video studio
            </Button>
          </div>
          {tab === "products" && (
            <div className="fashion-product-results">
              <h2>Product candidates</h2>
              <p>
                Source-reported details need review. Repeated frames do not
                independently verify product identity.
              </p>
              <div className="fashion-filters">
                <label>
                  Match
                  <select
                    value={matchFilter}
                    onChange={(e) => setMatchFilter(e.target.value)}
                  >
                    {[
                      "all",
                      "verified_exact",
                      "probable_exact",
                      "visually_similar",
                      "unidentified",
                    ].map((x) => (
                      <option key={x}>{x}</option>
                    ))}
                  </select>
                </label>
                <label>
                  Brand
                  <input
                    value={brand}
                    onChange={(e) => setBrand(e.target.value)}
                  />
                </label>
                <label>
                  Retailer
                  <input
                    value={retailer}
                    onChange={(e) => setRetailer(e.target.value)}
                  />
                </label>
                <label>
                  Category
                  <input
                    value={categoryFilter}
                    onChange={(e) => setCategoryFilter(e.target.value)}
                  />
                </label>
                <label>
                  Min USD
                  <input
                    type="number"
                    min="0"
                    value={minPrice}
                    onChange={(e) => setMinPrice(e.target.value)}
                  />
                </label>
                <label>
                  Max USD
                  <input
                    type="number"
                    min="0"
                    value={maxPrice}
                    onChange={(e) => setMaxPrice(e.target.value)}
                  />
                </label>
              </div>
              {!filteredProducts.length && (
                <p role="status">
                  {groupId
                    ? "No matching candidates. Search the catalog or review a ChatGPT research request."
                    : "Choose a garment group to inspect product candidates."}
                </p>
              )}
              {filteredProducts.map((c) => (
                <article key={c.id} className="fashion-product">
                  <div>
                    {c.product.image &&
                      ["owned", "copy_allowed"].includes(
                        c.product.imageRights,
                      ) && (
                        <img
                          src={api.image(c.product.image.path)}
                          alt={c.product.name}
                        />
                      )}
                    {c.product.imageAttribution&&<small>{c.product.imageAttribution}</small>}
                    {c.product.imageLicenseUrl&&<SourceLink url={c.product.imageLicenseUrl}>Image license</SourceLink>}
                  </div>
                  <div>
                    <h3>{c.product.name}</h3>
                    <p>
                      {c.product.brand ?? "Brand unknown"} ·{" "}
                      {c.classification.replaceAll("_", " ")}
                      {c.stale ? " · Evidence needs review" : ""}
                    </p>
                    <p>{c.reason}</p>
                    {c.listings.map((l) => (
                      <p key={l.id}>
                        {l.retailer} ·{" "}
                        {l.price && l.currency
                          ? `${l.currency} ${l.price}`
                          : "Price unknown"}{" "}
                        · {l.availability ?? "Availability unverified"}
                        {l.observedAt && (
                          <small>
                            Observed {listingObservation(l.observedAt)}
                          </small>
                        )}
                      </p>
                    ))}
                    <details>
                      <summary>Identification evidence</summary>
                      {c.evidence.map((e) => (
                        <div key={e.id}>
                          <SourceLink url={e.url}>{e.title}</SourceLink>
                          <blockquote>{e.excerpt}</blockquote>
                          <small>Evidence group: {e.independenceGroup}</small>
                        </div>
                      ))}
                      <label>
                        Visible garment/label identifier
                        <input
                          value={identifier}
                          onChange={(e) => {
                            setIdentifier(e.target.value);
                            setConfirmationId("");
                          }}
                        />
                      </label>
                      <label>
                        <input
                          type="checkbox"
                          checked={confirmationId === c.id}
                          onChange={(e) =>
                            setConfirmationId(e.target.checked ? c.id : "")
                          }
                        />
                        I checked this authoritative product source against the
                        visible garment or label identifier.
                      </label>
                      <Button
                        disabled={
                          c.stale || !identifier || confirmationId !== c.id
                        }
                        onClick={() =>
                          void perform(async () => {
                            await fashionApi.confirm(
                              groupId,
                              c.id,
                              identifier,
                              c.evidence[0]?.url ?? "",
                              confirmationId === c.id,
                            );
                            setProducts(await fashionApi.candidates(groupId));
                          })
                        }
                      >
                        Confirm matching identifier
                      </Button>
                    </details>
                    <div className="fashion-actions">
                      <SourceLink url={c.product.sourceUrl}>
                        Open product source
                      </SourceLink>
                      <Button
                        disabled={c.stale}
                        onClick={() => addToLook(groupId, c)}
                      >
                        Add product to look
                      </Button>
                      <Button
                        disabled={c.stale}
                        onClick={() => addToLook(groupId, c, true)}
                      >
                        Save alternative
                      </Button>
                    </div>
                  </div>
                </article>
              ))}
            </div>
          )}
          {tab === "looks" && (
            <div className="fashion-look-editor">
              <h2>Saved looks</h2>
              <label>
                Look
                <select
                  value={look.revision ? look.id : "new"}
                  onChange={(e) => {
                    const l = looks.find((l) => l.id === e.target.value);
                    setLook(l ? structuredClone(l) : freshLook(project));
                  }}
                >
                  <option value="new">New look</option>
                  {looks.map((l) => (
                    <option key={l.id} value={l.id}>
                      {l.name}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Name
                <input
                  value={look.name}
                  onChange={(e) =>
                    setLook((l) => ({ ...l, name: e.target.value }))
                  }
                />
              </label>
              <label>
                Notes
                <textarea
                  value={look.notes}
                  onChange={(e) =>
                    setLook((l) => ({ ...l, notes: e.target.value }))
                  }
                />
              </label>
              <ul>
                {look.items.map((i) => (
                  <li key={i.groupId}>
                    {groups.find((g) => g.id === i.groupId)?.name ?? "Garment"}{" "}
                    · {i.candidateId ? "Saved product" : "Unidentified item"} ·{" "}
                    {i.alternativeIds.length} alternatives{" "}
                    <Button
                      onClick={() =>
                        setLook((l) => ({
                          ...l,
                          items: l.items.filter((x) => x.groupId !== i.groupId),
                        }))
                      }
                    >
                      Remove from look
                    </Button>
                  </li>
                ))}
              </ul>
              <div className="fashion-actions">
                <Button
                  disabled={!look.name.trim() || running}
                  onClick={() =>
                    void perform(async () => {
                      setLook(await fashionApi.saveLook(look));
                      await refresh();
                      setNotice("Look saved.");
                    })
                  }
                >
                  <Save size={15} />
                  Save look
                </Button>
                <Button
                  disabled={!look.revision || running}
                  onClick={() =>
                    void perform(async () => {
                      await fashionApi.deleteLook(look.id);
                      setLook(freshLook(project));
                      await refresh();
                    })
                  }
                >
                  <Trash2 size={15} />
                  Delete look
                </Button>
              </div>
              <h3>Export lookbook</h3>
              <label>
                Format
                <select
                  value={format}
                  onChange={(e) => setFormat(e.target.value)}
                >
                  <option value="html">Self-contained HTML</option>
                  <option value="json">Structured JSON and assets</option>
                  <option value="reference">Reference pack</option>
                </select>
              </label>
              <Button
                disabled={!look.revision || running}
                onClick={() =>
                  void perform(async () => {
                    const p = await open({ directory: true, multiple: false });
                    if (typeof p === "string")
                      track(
                        await fashionApi.exportLook(
                          project,
                          look.id,
                          p,
                          format,
                        ),
                      );
                  })
                }
              >
                Export saved look
              </Button>
              {exports.map((e) => (
                <div className="fashion-export-row" key={e.id}>
                  <span>
                    {e.look.name} · {e.status} · {e.completed.length}/
                    {e.files.length}
                  </span>
                  <Button
                    disabled={running}
                    onClick={() =>
                      void perform(async () =>
                        track(await fashionApi.resumeExport(e.id)),
                      )
                    }
                  >
                    Verify / resume export
                  </Button>
                </div>
              ))}
            </div>
          )}
        </div>
        <aside className="fashion-dock">
          <section>
            <h2>Items</h2>
            {lastDeleted && (
              <Button
                disabled={running}
                onClick={() =>
                  void perform(async () => {
                    await fashionApi.undoItem(lastDeleted);
                    setLastDeleted("");
                    await refresh();
                  })
                }
              >
                Restore deleted item
              </Button>
            )}
            <div className="fashion-actions">
              <Button disabled={!asset || running} onClick={newItem}>
                Draw item
              </Button>
              <Button
                disabled={
                  !media ||
                  running ||
                  !capabilities.some(
                    (c) => c.task === "detection" && c.status !== "unavailable",
                  )
                }
                onClick={() =>
                  void perform(async () =>
                    track(await fashionApi.detect(project, media!)),
                  )
                }
              >
                <Scan size={15} />
                Detect locally
              </Button>
            </div>
            {!capabilities.some(
              (c) => c.task === "detection" && c.status !== "unavailable",
            ) && (
              <p>
                Local detection needs a fashion task pack. Draw regions or use
                reviewed ChatGPT detection below.
              </p>
            )}
            <ul className="fashion-item-list">
              {visible.map((i) => (
                <li key={i.id}>
                  <input
                    type="checkbox"
                    aria-label={`Group ${i.category} observation`}
                    checked={chosen.includes(i.id)}
                    onChange={(e) =>
                      setChosen((s) =>
                        e.target.checked
                          ? [...s, i.id]
                          : s.filter((id) => id !== i.id),
                      )
                    }
                  />
                  <button
                    aria-pressed={selected === i.id}
                    onClick={() => {
                      setSelected(i.id);
                      setMode("inspect");
                    }}
                  >
                    {i.crop && <img src={api.image(i.crop.path)} alt="" />}
                    <span>
                      {i.category.replaceAll("_", " ")}
                      <small>
                        {i.corrected ? "User corrected" : i.provenance}
                      </small>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
          {draft && (
            <section>
              <h3>{draft.id ? "Correct item" : "New item"}</h3>
              <label>
                Category
                <select
                  value={draft.category}
                  onChange={(e) =>
                    setDraft((d) =>
                      d ? { ...d, category: e.target.value } : d,
                    )
                  }
                >
                  {categories.map((c) => (
                    <option key={c} value={c}>
                      {c.replaceAll("_", " ")}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Region tool
                <select value={mode} onChange={(e) => setMode(e.target.value)}>
                  <option value="inspect">Inspect</option>
                  <option value="box">Draw / move box</option>
                  <option value="polygon">Polygon points</option>
                  <option value="outline">Trace mask outline</option>
                </select>
              </label>
              <div className="fashion-coordinates">
                {(["x", "y", "width", "height"] as const).map((k) => (
                  <label key={k}>
                    {k}
                    <input
                      type="number"
                      min="0"
                      max="1"
                      step="0.001"
                      value={draft.rectangle[k]}
                      onChange={(e) =>
                        setDraft((d) =>
                          d
                            ? {
                                ...d,
                                polygon: [],
                                rectangle: {
                                  ...d.rectangle,
                                  [k]: Number(e.target.value),
                                },
                              }
                            : d,
                        )
                      }
                    />
                  </label>
                ))}
              </div>
              <details>
                <summary>Polygon coordinates</summary>
                <textarea
                  aria-label="Polygon coordinates"
                  value={JSON.stringify(draft.polygon)}
                  onChange={(e) => {
                    try {
                      const p = JSON.parse(e.target.value);
                      if (
                        Array.isArray(p) &&
                        p.length <= 128 &&
                        p.every(
                          (x) =>
                            Array.isArray(x) &&
                            x.length === 2 &&
                            x.every((v) => typeof v === "number"),
                        )
                      )
                        setDraft((d) => (d ? { ...d, polygon: p } : d));
                    } catch {}
                  }}
                />
              </details>
              <Button
                onClick={() => setDraft((d) => (d ? { ...d, polygon: [] } : d))}
              >
                Clear outline
              </Button>
              <label>
                Attributes · name: value
                <textarea
                  rows={5}
                  value={attributes}
                  onChange={(e) => setAttributes(e.target.value)}
                />
              </label>
              {current?.attributes.some((a) => a.origin === "inferred") && (
                <p>
                  Some attributes are inferred. Review before saving them as
                  user corrections.
                </p>
              )}
              <label>
                <input
                  type="checkbox"
                  checked={showMask}
                  onChange={(e) => setShowMask(e.target.checked)}
                />
                Inspect saved mask
              </label>
              <div className="fashion-actions">
                <Button disabled={running} onClick={() => void saveItem()}>
                  <Save size={15} />
                  Save correction
                </Button>
                {draft.id && (
                  <>
                    <Button
                      disabled={running}
                      onClick={() =>
                        void perform(async () => {
                          await fashionApi.undoItem(draft.id);
                          await refresh();
                        })
                      }
                    >
                      <Undo2 size={15} />
                      Undo
                    </Button>
                    <Button
                      disabled={running}
                      onClick={() =>
                        void perform(async () => {
                          await fashionApi.deleteItem(draft.id);
                          setLastDeleted(draft.id);
                          setDraft(null);
                          setSelected("");
                          await refresh();
                        })
                      }
                    >
                      <Trash2 size={15} />
                      Delete
                    </Button>
                    <Button
                      disabled={
                        running ||
                        !capabilities.some(
                          (c) =>
                            c.task ===
                              (draft.category === "hair" ? "hair" : "mask") &&
                            c.status !== "unavailable",
                        )
                      }
                      onClick={() =>
                        void perform(async () =>
                          track(await fashionApi.refineMask(draft.id)),
                        )
                      }
                    >
                      Refine local mask
                    </Button>
                  </>
                )}
              </div>
            </section>
          )}
          <section>
            <h2>Garment views</h2>
            <label>
              Group
              <select
                value={groupId}
                onChange={(e) => {
                  setGroupId(e.target.value);
                  const g = groups.find((g) => g.id === e.target.value);
                  setGroupName(g?.name ?? "Garment");
                  setChosen(g?.itemIds ?? []);
                }}
              >
                <option value="">New group</option>
                {groups
                  .filter((g) => g.itemIds.length)
                  .map((g) => (
                    <option key={g.id} value={g.id}>
                      {g.name} · {g.itemIds.length} views
                    </option>
                  ))}
              </select>
            </label>
            <label>
              Name
              <input
                value={groupName}
                onChange={(e) => setGroupName(e.target.value)}
              />
            </label>
            <p>{chosen.length} checked observations across this project.</p>
            {conflicts.length > 0 && (
              <details>
                <summary>Conflicting observations — review</summary>
                <ul>
                  {conflicts.map(([name, values]) => (
                    <li key={name}>
                      {name}: {values.join(" / ")}
                    </li>
                  ))}
                </ul>
                <p>
                  All conflicting attributes remain attached to their source
                  observations.
                </p>
              </details>
            )}
            {chosen.map((id) => {
              const i = items.find((i) => i.id === id);
              return i ? (
                <button
                  className="text-button"
                  key={id}
                  onClick={() => {
                    if (same(media, i.media)) setSelected(i.id);
                    else {
                      pendingSelection.current = i.id;
                      setMedia(i.media);
                    }
                  }}
                >
                  {i.category} ·{" "}
                  {i.media.kind === "frame"
                    ? `frame ${i.media.selection.frameRef.presentationOrdinal}`
                    : "photo"}
                </button>
              ) : null;
            })}
            <div className="fashion-actions">
              <Button
                disabled={!chosen.length || running}
                onClick={() => void saveGroup()}
              >
                Save / merge checked views
              </Button>
              <Button
                disabled={!chosen.length || running}
                onClick={() => void saveGroup(true)}
              >
                Split into new group
              </Button>
              <Button
                disabled={!items.length || running}
                onClick={() =>
                  void perform(async () => {
                    track(await fashionApi.associate(project));
                    await refresh();
                  })
                }
              >
                Suggest associations
              </Button>
              <Button
                disabled={!groupId}
                onClick={() => addToLook(groupId, null)}
              >
                Add unknown item to look
              </Button>
            </div>
          </section>
          <section>
            <h2>AI analysis and research</h2>
            <details>
              <summary>How to build a look</summary>
              <ol>
                <li>Import a photo or select exact video frames.</li>
                <li>
                  Draw or detect items, then correct their regions and
                  observable attributes.
                </li>
                <li>
                  Check observations and save a garment group. Keep conflicting
                  details for review.
                </li>
                <li>
                  Review the exact images before research. Inspect product
                  sources and identifying evidence.
                </li>
                <li>
                  Add products, alternatives or unknown items to a look, save
                  it, then export.
                </li>
              </ol>
              <p>
                Keyboard users can edit box coordinates, choose polygon points,
                and operate every dock control without dragging.
              </p>
            </details>
            <label>
              Engine
              <select
                value={runtime}
                onChange={(e) => {
                  setRuntime(e.target.value);
                  setModels([]);
                  setModel("");
                  if (e.target.value === "lm_studio") setPort(1234);
                  else setPort(11434);
                }}
              >
                <option value="">ChatGPT subscription</option>
                <option value="ollama">Local Ollama</option>
                <option value="lm_studio">Local LM Studio</option>
              </select>
            </label>
            {runtime && (
              <label>
                Loopback port
                <input
                  type="number"
                  min="1"
                  max="65535"
                  value={port}
                  onChange={(e) => setPort(Number(e.target.value))}
                />
              </label>
            )}
            <div className="fashion-actions">
              <Button disabled={running} onClick={() => void loadModels()}>
                Load available models
              </Button>
              <Button
                onClick={() => {
                  useSettingsTab
                    .getState()
                    .setTab(runtime ? "models" : "cloud");
                  w.setView("settings");
                }}
              >
                Connection settings
              </Button>
            </div>
            <label>
              Model
              <select value={model} onChange={(e) => setModel(e.target.value)}>
                <option value="">Choose a model</option>
                {models.map((m) => (
                  <option key={m}>{m}</option>
                ))}
              </select>
            </label>
            <label>
              Task
              <select
                value={task}
                onChange={(e) => setTask(e.target.value as FashionTaskV1)}
              >
                <option value="detect">Find items in this image</option>
                <option value="attributes">Describe selected item</option>
                <option value="research" disabled={!!runtime}>
                  Research grouped garment online
                </option>
              </select>
            </label>
            <label hidden={task !== "research"}>
              Shopping provider
              <select
                value={shoppingProvider}
                onChange={(e) => setShoppingProvider(e.target.value)}
              >
                <option value="chatgpt">ChatGPT web search</option>
                <option value="ebay">
                  Optional authorized eBay image search
                </option>
              </select>
            </label>
            <p>
              {task === "research"
                ? "Search capability is unverified for this account/model until a reviewed request completes. ChatGPT uses hosted web search and retains consulted sources. Unsupported accounts can use local catalog or manual source links."
                : "Model suggestions are unverified. Correct regions and attributes before relying on them."}
            </p>
            <Button
              disabled={
                running ||
                !asset ||
                (!(task === "research" && shoppingProvider === "ebay") &&
                  !model) ||
                (task === "attributes" && !selected) ||
                (task === "research" && !groupId)
              }
              onClick={() => void prepare()}
            >
              Prepare image review
            </Button>
            {review && (
              <div className="fashion-review">
                <h3>Review exact images</h3>
                <p>
                  {review.request.runtime
                    ? "These images go to the selected loopback server."
                    : "These images go to the selected research provider. ChatGPT requests may consume your plan allowance; optional eBay requires authorized credentials."}
                </p>
                {review.images.map((a) => (
                  <img
                    key={a.path}
                    src={api.image(a.path)}
                    alt="Exact image prepared for AI analysis"
                    onLoad={() =>
                      setLoaded((s) => [...new Set([...s, a.path])])
                    }
                  />
                ))}
                <label>
                  <input
                    type="checkbox"
                    checked={consent}
                    onChange={(e) => setConsent(e.target.checked)}
                  />
                  I approve this image-analysis request.
                </label>
                <Button
                  disabled={
                    running ||
                    !consent ||
                    loaded.length !== review.images.length
                  }
                  onClick={() =>
                    void perform(async () => {
                      const id = review.id;
                      const job = await fashionApi.run(id, true);
                      setReview(null);
                      setConsent(false);
                      track(job);
                    })
                  }
                >
                  Send reviewed images
                </Button>
                <Button
                  onClick={() => {
                    void fashionApi.discard(review.id);
                    setReview(null);
                  }}
                >
                  Discard review
                </Button>
              </div>
            )}
          </section>
          <section>
            <FashionAnalysisHistory
              project={project}
              revision={currentJob?.status}
            />
            <h2>Local catalog</h2>
            <Button
              disabled={running}
              onClick={() =>
                void perform(async () => {
                  const p = await open({
                    multiple: false,
                    filters: [{ name: "Catalog", extensions: ["json", "csv"] }],
                  });
                  if (typeof p === "string")
                    track(await fashionApi.importCatalog(p));
                })
              }
            >
              <FolderOpen size={15} />
              Import catalog
            </Button>
            <label>
              Catalog search
              <input value={query} onChange={(e) => setQuery(e.target.value)} />
            </label>
            <Button
              disabled={!groupId || running}
              onClick={() =>
                void perform(async () => {
                  setProducts(await fashionApi.searchCatalog(groupId, query));
                  setTab("products");
                })
              }
            >
              Find catalog candidates
            </Button>
          </section>
        </aside>
      </div>
    </section>
  );
}
function SourceLink({
  url,
  children,
}: {
  url: string;
  children: React.ReactNode;
}) {
  return (
    <a
      href={url}
      target="_blank"
      rel="noreferrer"
      onClick={(e) => {
        if (desktop) {
          e.preventDefault();
          void openUrl(url);
        }
      }}
    >
      {children}
      <ExternalLink size={12} />
    </a>
  );
}

function listingObservation(value: string) {
  const date = new Date(/^\d+$/.test(value) ? Number(value) * 1000 : value);
  if (!Number.isFinite(date.getTime())) return "time unavailable";
  const old = Date.now() - date.getTime() > 7 * 86400000;
  return `${date.toLocaleString()}${old ? " · older listing; check source for current price and availability" : ""}`;
}
