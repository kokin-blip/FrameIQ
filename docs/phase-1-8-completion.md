# FrameIQ phases 1–8 implementation ledger

This ledger covers the user-approved fashion buildout. Phase 9 is excluded: no Windows acceptance program, GPU acceleration, sustained profiling, large accuracy corpus, public signing/notarization or release packaging. The original extractor, `FrameRefV1`, saved projects, Fluent tokens and ChatGPT subscription bridge remain in place. Existing AV1 and branding work is preserved.

## Feature matrix

“Implemented” means the workflow, persistence and failure handling exist and have focused verification. “Experimental” means execution exists, but suitability is limited to the recorded adapter checks and examples. “Unavailable” means a required pack failed qualification or has not been installed. “Externally blocked” means live account/provider access is needed for acceptance; it is never presented as a successful live check.

| Requirement | State | Implementation and evidence |
|---|---|---|
| Subscription reconciliation | Implemented | Bridge from `d9efe7d` reused; account-bound reviewed requests; no paid API fallback |
| Editable Fluent compositions | Implemented; renderer check limited | Existing Figma file, six workflow foundations plus fifteen editable empty/loading/unavailable/error/correction compositions; structural bounds checked; screenshot service timed out on the final expanded states |
| JPEG/PNG/WebP photos | Implemented | Native orientation-aware decoding, photo revisions/heads, persistent mixed-media projects, source checks and library covers; photos never create video-frame identities |
| Shared inspection | Implemented | Central canvas, photo information/exact-frame information, synchronized item list and cross-media group navigation |
| Task packages | Experimental | Versioned typed/bounded multi-input profiles, auxiliary hashes, preprocessing and adapter versions; strict graph checks, staged reviewed installation, cancellation, supervised sessions and removal/invalidation |
| Local task execution | Experimental | Detection, hair, prompted mask and embedding adapters; task result cache includes image digest, region, model/artifacts, preprocessing and adapter version |
| Local vision attributes | Experimental; live model comparison external | Actual Ollama/LM Studio reviewed-image requests, capability checks, cloud-model rejection, structured parsing; HTTP protocol tests pass |
| Task routing / comparison | Implemented; accuracy selection requires review | ChatGPT defaults for contextual attributes/research, local spatial/retrieval tasks, per-task Settings preferences and same-input hashes in analysis history. No claim that an untested local model is stronger than ChatGPT |
| Fashion categories | Experimental | Original garment/accessory/footwear category prompts, individual observations, scores where supplied, source references, crops and masks |
| Corrections and undo | Implemented | Box/polygon/outline editing, relabel/delete/restore/undo, optimistic revisions. Unchanged attribute origins/masks survive edits; removed attributes remain suppressed; reruns respect corrected/deleted regions |
| Rich attributes | Experimental | Observed/inferred/user origins, uncertainty, colors/patterns/neckline/sleeves/hemline/silhouette/texture/logos/hardware/footwear/bag details, retained analysis provenance |
| All seven presets | Experimental | Makeup, Hairstyle, Outfit, Complete Look, Accessories, Footwear, Custom Region; dependency checks; every displayed weight scored; explicit geometry proxies when pose signals are missing |
| Tracking and refinement | Experimental | Bidirectional anchor tracking, shot/subject/pose ambiguity stops, target-guided exact neighboring-frame refinement, cached reranking, exact source restoration |
| Complete Look | Experimental | Complementary supported view buckets, fewer results when views are missing, optional detected detail crops, source-linked inspection and dependency invalidation |
| ChatGPT product research | Implemented; live acceptance externally blocked | Reviewed group crops/context, hosted web search, completed search events/citations/URLs, bounded structured normalization, unsupported-capability state and no-match history |
| Local catalog | Implemented; visual retrieval experimental | JSON/CSV metadata and permitted image import, text/category retrieval, qualified local embeddings, separate timestamped listings and unknown prices |
| Optional eBay | Implemented; live acceptance externally blocked | Authorized production credentials in OS storage, image-search provider interface, typed evidence, access/quota failures, no purchasing or paid fallback |
| Identity and product results | Implemented; candidate quality experimental | Separate products/listings/evidence, filters, quoted identifying evidence, probable vs visually similar, exact confirmation requires visible matching identifier plus explicit authoritative-source attestation |
| Multi-frame garments | Experimental associations; manual controls implemented | Unique subject-track/category/geometry/appearance/embedding suggestions, ambiguity abstention, merge/split/reassignment, conflicting attributes retained, complementary reviewed views, duplicate research prevention |
| Evidence independence | Implemented conservative rules | Identical reviewed crops deduplicated; repeated frames and provider agreement never promote identity; source groups recorded, similarity cannot verify identity |
| Saved looks | Implemented | Save/rename/edit/reopen/remove, products, alternatives, unresolved garments and notes with evidence retained |
| Fashion exports / Export Studio | Implemented | Frozen JSON/HTML/reference snapshots, permitted source/crop assets, retailer links, rights-aware product images, ownership/collision checks, cancellation, verified resume and preserved modified outputs |
| Help / readiness | Implemented | In-workflow guide, keyboard coordinates, Settings task readiness/preferences, explicit unavailable states and this matrix |
| Migration / compatibility | Implemented | Checksummed migration 012 uses existing backup mechanism; original migrations/frame contract untouched; relations/searchable fields normalized and provenance versioned |

## Verification

- Full native suite with fault injection: **115 passed, 0 failed, 7 intentionally ignored**. This covers existing extraction/VFR/fidelity/export/recovery tests, model lifecycle, local HTTPS fixtures, tracking and the new fashion workflows. Existing opt-in public-model/live-runtime tests remain ignored by default.
- All seven preset orchestration paths pass with deterministic protocol doubles, including a middle anchor, both tracking directions, detail crops, missing-view behavior, cached reranking, cancellation and pose-removal invalidation. These tests establish workflow correctness, not model accuracy.
- All four qualified packs pass actual application import/removal; SAM and hair also pass real refinement, undo and preservation after model removal.
- Five fashion integration tests cover photo orientation/reimport, source changes, mixed media, corrections/undo, group splitting, false exact-match prevention, saved looks, HTML escaping, immutable exports/reopening/conflicts, review tampering/scope and opt-in real task-pack installation/removal.
- Additional correction regression verifies that unchanged inferred attributes and masks keep their provenance, explicit attribute removals survive analysis, and user color corrections are protected.
- Real loopback request tests cover Ollama image payloads, LM Studio image content and rejection of cloud-marked Ollama models. No private images or live external generation were used.
- Frontend: **55 tests pass**; contracts generation, TypeScript and production build pass. Desktop compilation passes. The desktop subscription-vault/account-binding regression also passes.
- Actual Fashion workspace checked at **1440×960** and **1000×700** with clearly labeled UI fixture data. Item selection, numeric corrections, independently scrolling docks, empty product results, saved-look selection and keyboard-editable notes were inspected. Compact document width equals viewport width. A hidden-control CSS defect was fixed and rechecked.

## Model qualification

Reports and small manifests belong in `docs/models/fashion-qualification/`; weights stay outside Git. The four qualified packs are available as reviewed direct downloads in onboarding and **Settings → Local models → Optional fashion packs**. The pinned catalog lives in `models/task-packs/`, with converted artifacts and notices hosted in the versioned `fashion-packs-v1` model-asset release. Advanced local import remains available in Settings, and development packages remain under `.cache/fashion-qualification/`. Normal application inference uses the bundled worker and ONNX Runtime; Python tools are development-only.

- **GroundingDINO-T:** pinned upstream export, static category tokens, bounded multi-input graph, eager attention and top-left letterboxing. Four representative images pass PyTorch/ONNX task-output matching; five native checks additionally cover a CC0 full-shoe photograph, classified as footwear. Raw query tensor differences are retained because proposal ordering can differ; thresholded class/box/score comparisons determine parity. Arbitrary-import validation is unchanged; reviewed graphs have a bounded operator/node allowance for the actual export.
- **MediaPipe HairSegmenter:** original candidate cannot run in the ONNX worker due to custom TFLite pool/unpool operators. It stays unavailable. The official **MediaPipe multiclass 256 model**, Apache-2.0, is the compatible hair alternative. Its declared normalization is read from model metadata, original TFLite logits are compared with ONNX, and native hair masks pass four checks. Inference can use a reviewed head ROI and projects the mask back to the complete source.
- **SigLIP 2 Base:** PyTorch/ONNX and native normalized embedding comparisons pass; model quality remains experimental.
- **SAM 2.1 Tiny:** export/reference and native mask comparisons did not satisfy all gates. Its graph was simplified to eliminate control-flow subgraphs instead of weakening runtime validation. It remains unavailable; manual polygon/outline masks work. The original **SAM ViT-B** alternative passes the PyTorch/ONNX prompted-mask comparisons and all four native mask checks with the compatible resize profile. Import its `sam-base` task package for prompted refinement; SAM 2 remains gated.

The small reference inputs include a NASA public-domain portrait and three frames from the already reviewed Berlin ethical-fashion clip (CC BY 3.0, GermanFashion.TV). Source SHA-256, upstream commit, exported model digest, tensors/decisions and numerical errors are retained. These correlated examples establish adapter behavior only. A separate CC0 shoe detail and full-shoe photograph have per-file provenance in `docs/models/fashion-qualification/example-sources.json`; the full shoe is included in detection and embedding checks. These are not an annotated garment/hair/accessory/footwear accuracy corpus or evidence of independent product identity. See `tests/fixtures/images/README.md` and `tests/evaluation/public-sources.json` for provenance.

## External acceptance and defaults

A live eligible ChatGPT account/model, compatible installed local vision server, and authorized eBay production access were not available for comparative/live acceptance. The app exposes their actual failures and preserves local/manual work. Search is account/model-policy dependent; no undocumented backend or paid API route is used. See the official [subscription limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations) and [inference guidance](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference).

Defaults remain US/USD, optional external analysis, exact reviewed-image consent, local provider/account request quotas, OS credential storage and a $0 third-party-service ceiling. No checkout, purchase automation, mobile flow or phase 9 distribution work is included.

## Design references

[Editable Fluent Figma file](https://www.figma.com/design/5L64rf754pinHPHimkme5e): primary compositions `51:225`, `51:238`, `51:251`, `51:264`, `51:277`, `51:290`. Expanded state roots: Items `55:247`, `55:277`, `55:307`, `55:337`, `55:367`; Products `55:413`, `55:443`, `55:473`, `55:503`, `55:533`; Looks `55:579`, `55:609`, `55:639`, `55:669`, `55:699`. Layouts use editable text, hierarchy and Fluent button instances. They contain no fabricated retailer facts or flattened UI images.

## Catalog format

Use a JSON array or CSV with `name`, `category`, `sourceUrl` and optional `brand`, `styleId`, `imagePath`, `imageRights`, `imageAttribution`, `imageLicenseUrl`, `retailer`, `price`, `currency`, `availability`, `observedAt`. Image paths must stay inside the catalog directory. `imageRights` is `owned`, `copy_allowed` or `link_only`; copying is disabled by default. Price/availability require a source observation timestamp; unknown prices stay unknown. Only permitted images are copied, and recorded attribution/license links travel with exports.

Preprocessing profiles explicitly declare their resize algorithm. `pillow_bilinear_v1` matches the development reference's separable byte rounding in native Rust; a known-pixel regression checks it. Legacy task profiles retain their existing resize behavior. Qualification reports are bound to the preprocessing digest as well as the model digest.
