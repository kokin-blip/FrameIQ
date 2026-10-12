# OpenAI and Gemini — 2026-10-11

Scope: user-supplied credentials, explicit connection tests, descriptions of one exact frame or crop, and advisory ranking of 2–8 explicitly chosen frames. Existing offline extraction and local preset measurements remain independent. No SQLite migration or change to FrameRefV1. Release v0.1.0 assets are unchanged.

## Credentials and transport

Keys live in provider-specific OS credential entries under `com.frameiq.cloud`. Status returns only presence; saved keys never return to the frontend or enter projects, exports, or logs. Newly entered password-field values clear after successful saving/removal. Saving and startup do not contact either provider.

Explicit connection tests list models using OpenAI `/v1/models` or Gemini `/v1beta/models`, with sensitive authorization headers. They send no images or generation prompts. Counts describe the returned page, and pagination is disclosed. A listed model does not establish image/structured-output compatibility or funded access.

Requests use fixed HTTPS hosts, normal TLS verification, no redirects or ambient proxies, a five-second connection timeout, and bounded two-MiB responses. Metadata tests have a fifteen-second request timeout; analysis has ninety seconds. Errors exclude provider bodies, keys, and transport diagnostics. There are no automatic paid retries or provider fallbacks.

## Review and analysis

Studio's Cloud AI panel offers OpenAI/Gemini, an explicit image-capable model ID, task, and preset goal. It describes the current indexed frame or normalized crop; ranking accepts 2–8 chosen collection frames across sources within one project. Preset goals steer qualitative suggestions rather than alter calibrated local scores or enable unavailable automatic capabilities.

Preparation is local and cancellable. Exact frame references and source content are validated, then JPEG previews are generated with fresh metadata, quality 85, no enlargement, a 1,024-pixel maximum dimension, and one-MiB per-image cap. The user reviews those exact images and authorizes their transmission. Send remains disabled until every preview loads and consent is checked. Changing request inputs invalidates the review. Reviews are single-use, bounded to four, expire after ten minutes, and are discarded when abandoned. Sources and image hashes are checked again before dispatch.

OpenAI uses inline images in Responses with `store:false`; Gemini uses inline images in generateContent. No provider file upload, search tool, or cloud catalog is used. Provider account data-use terms still apply; this implementation does not promise zero retention. Visible faces/backgrounds can remain in reviewed images. Output is capped at 1,200 tokens for description or 3,000 for ranking, but charges are not represented as a fixed price.

Structured assessments separate observed details, uncertainty, and reasoning. Unknown/duplicate image IDs, incomplete rank permutations, excess fields, oversized strings, refusal, and incomplete responses are rejected. Results are labeled unverified. They retain exact transmitted previews and frame provenance, support opening/selecting those frames, and show reported model/token usage when available. They do not establish brands, product identity, material composition, or physical quality probabilities.

One cloud lane bounds execution. Cancellation drops the in-flight request; already transmitted data cannot be recalled and may be billed. Successful result JSON and application-owned image directories are cached locally, capped at sixteen result sets. Startup cleans marked abandoned preparations/orphans while preserving unknown files. Expired results never trigger another paid request automatically. The current UI does not provide a persistent cloud-history browser.

## Verification

- Full Rust workspace regression/fault suite passed, excluding the desktop test target; the native shell compiles through the production build.
- Nine provider unit tests cover endpoints/headers, safe errors, pagination, structured payloads, refusal/incomplete responses, output identity/ranks, bounds, and pre-dispatch cancellation.
- Application tests use real decoded FFV1 media to check exact ordinal/crop, JPEG dimensions and checksum, no-consent rejection, source/stale-reference validation, tamper rejection before network access, and single-use cleanup. Cleanup tests preserve unknown files and enforce the sixteen-result limit.
- All 28 frontend tests in nine files passed, including explicit credentials/actions, loaded-preview consent, input invalidation, chosen-frame ranking, result uncertainty, exact-frame opening, and selection integration.
- TypeScript, generated-contract checks, frontend production build, and macOS app build passed. An isolated preview uses a distinct application identifier/data directory. No production project or released binary was replaced.
- Native no-key/empty Studio and Settings captures cover 1000×700 and 1440×886 logical sizes. Settings scrolls independently. The available display prevents a 1440×960 native capture. A bounded independent visual review identified native select sizing; full-width 36px Fluent controls replace compact native rendering. The independent confirmation closed that defect at both captured sizes, with no remaining material issue in the scored correction. This disposition covers captured no-key states only.
- Editable Figma credential and analysis states precede production controls; state ledgers live in `docs/design/`.

Evidence logs are retained in the session's `/private/tmp/frameiq-cloud-*.log`; native review captures are local-only under `.impeccable/review/cloud/`.

## Open acceptance

No real API keys were entered or provider generation performed. Successful authentication/generation, provider-specific model compatibility, account billing behavior, response accuracy, OS credential save/read/delete round trips, populated native interaction, and Windows cloud acceptance remain pending. Unit/component fixtures are not live provider acceptance. Cloud advice does not close existing automatic preset accuracy or local-model feasibility gates.

## Official references

- [OpenAI model listing](https://developers.openai.com/api/reference/resources/models/methods/list)
- [OpenAI image input](https://developers.openai.com/api/docs/guides/images-vision)
- [OpenAI structured output](https://developers.openai.com/api/docs/guides/structured-outputs?api-mode=responses)
- [Gemini model listing](https://ai.google.dev/api/models)
- [Gemini image input](https://ai.google.dev/gemini-api/docs/image-understanding)
- [Gemini structured output](https://ai.google.dev/gemini-api/docs/structured-output)
