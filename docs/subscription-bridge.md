# ChatGPT subscription bridge

FrameIQ defaults to a bring-your-own-subscription (BYOS) ChatGPT connector.
Connect it in **Settings → AI connections → Continue with ChatGPT**, or use
**Connect ChatGPT** during model setup. OpenAI and Gemini API-key connectors,
Ollama/LM Studio, and managed local models remain available.

The current bridge runs the existing reviewed-image description and ranking
steps against the selected account's model catalog. All preset goals are
supported, including makeup, hairstyle, outfit, complete look, accessories,
footwear, and a custom region. This avoids separate model downloads for those
reasoning requests. Local extraction, person/face detection, tracking, pose
landmarks, masks, and quality measurements retain their existing implementations.
The bridge does not launch a remote ChatGPT Work session or replace deterministic
local inference outputs with unverified model text.

## Authentication and requests

- Native Authorization Code + PKCE/S256 with a random state and OIDC nonce.
  A loopback listener starts on `127.0.0.1` before the system browser opens.
- First-time local registration uses the documented `dynamic_agent_client`
  entry point. A stable host identifier and the issued account/client mapping
  survive sign-out. Deployments with an issued public OAuth client can set
  `FRAMEIQ_OPENAI_CLIENT_ID` in the native app's environment before launch.
  Commercial availability remains subject to OpenAI client provisioning.
- ID tokens require a JWKS-verified signature, issuer, audience, expiration,
  subject, issued-at, and matching nonce. Plan usage requires the granted
  `resource.invoke` and `chatgpt.tokens.use.direct` scopes.
- Each account registration has a distinct label and identity. Returning login
  reuses its client ID and verifies the same subject before replacing tokens.
  Images reviewed under one account cannot be sent after switching accounts
  without preparing a new review.
- Tokens live in native OS credential storage, in versioned bounded chunks with
  an atomic manifest commit to accommodate Windows vault limits. The webview,
  project database, preferences, exports, and logs never receive tokens. Refresh
  rotation is serialized. Sign-out attempts remote revocation and clears the
  local session, reporting when remote revocation cannot be confirmed.
- Model discovery calls the public `/v1/models` endpoint, keeps server ordering,
  displays account catalog names, and sends the corresponding model slugs.
- Image requests call the public `/v1/responses` endpoint with `store:false` and
  `stream:true`. Unsupported subscription fields such as `max_output_tokens`
  are omitted. Responses are bounded locally to 2 MiB and 90 seconds; these are
  transport bounds, not a provider-side token cap. Partial/failed streams never
  become suggestions; `response.completed` and validated assessments are required.
- Existing exact-image preview, crop, hash validation, consent, cancellation,
  and provenance handling apply to BYOS requests. Sign-in and connection tests
  send no images. A subscription failure never triggers an API-key fallback.
  Requests are not automatically retried and may consume plan usage/credits
  even if interrupted. Users manage access and usage in ChatGPT Settings.

## Verification and limits

Automated coverage verifies identity signatures/claims, callback substitution,
PKCE, consent boundaries, account-specific model listing, SSE byte boundaries,
incomplete/failed streams, output bounds, refresh rotation, sign-in cancellation,
account selection, and preservation of existing API-key behavior.

Live OpenAI sign-in, eligibility, quota behavior, and inference require a user
account and have not been exercised by these tests. The documented Sign in with
ChatGPT flow is in preview; available accounts, models, and capabilities are
controlled by OpenAI. Hosted ChatGPT tools and raw video input are outside this
bridge's reviewed-image workflow. Native macOS compilation is checked; Windows
credential storage and browser behavior still need device verification.

Official references:

- [Registration and sign-in](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
- [Accounts and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)
- [Models and inference](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)
- [Preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
