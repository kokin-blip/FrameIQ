//! Sign in with ChatGPT public-client OAuth. Secrets stay in the native process.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use frameiq_contracts::*;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use reqwest::{blocking::Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const ISSUER: &str = "https://auth.openai.com";
const RESOURCE: &str = "https://api.openai.com/v1";
const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
const SCOPE: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

fn error(code: &str, message: &str) -> AppError {
    AppError::new(code, message)
}
fn auth_error() -> AppError {
    error(
        "SUBSCRIPTION_AUTH",
        "ChatGPT sign-in could not be verified. Continue with ChatGPT again.",
    )
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random() -> String {
    format!(
        "{}{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn client() -> AppResult<Client> {
    Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| auth_error())
}
fn json(response: reqwest::blocking::Response) -> AppResult<Value> {
    if !response.status().is_success() {
        return Err(error(
            "SUBSCRIPTION_AUTH",
            "ChatGPT authorization failed. Check access in ChatGPT Settings and sign in again.",
        ));
    }
    let mut bytes = Vec::new();
    response
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| auth_error())?;
    if bytes.len() > 256 * 1024 {
        return Err(auth_error());
    }
    serde_json::from_slice(&bytes).map_err(|_| auth_error())
}
fn discovery(client: &Client) -> AppResult<Value> {
    let value = json(
        client
            .get(format!("{ISSUER}/.well-known/openid-configuration"))
            .send()
            .map_err(|_| auth_error())?,
    )?;
    if value["issuer"] != ISSUER {
        return Err(auth_error());
    }
    Ok(value)
}
fn auth_endpoint(value: &Value, key: &str) -> AppResult<Url> {
    let url = Url::parse(value[key].as_str().ok_or_else(auth_error)?).map_err(|_| auth_error())?;
    if url.scheme() != "https"
        || url.host_str() != Some("auth.openai.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(auth_error());
    }
    Ok(url)
}

// These records are serialized only into OS credential storage, never IPC or project files.
#[derive(Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: u64,
    pub scopes: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub client_id: String,
    pub subject: String,
    pub email: Option<String>,
    pub tokens: Option<Tokens>,
}
impl Account {
    pub fn public(&self) -> SubscriptionAccountV1 {
        SubscriptionAccountV1 {
            id: self.id.clone(),
            label: format!(
                "{} · {}",
                self.email.as_deref().unwrap_or("ChatGPT account"),
                self.id.get(..8).unwrap_or(&self.id)
            ),
            connected: self.tokens.is_some(),
        }
    }
    pub fn refresh_if_needed(&mut self) -> AppResult<()> {
        let tokens = self.tokens.as_ref().ok_or_else(auth_error)?;
        if tokens.expires_at > now() + 60 {
            return Ok(());
        }
        let response = client()?
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", &self.client_id),
                ("refresh_token", &tokens.refresh_token),
                ("resource", RESOURCE),
            ])
            .send()
            .map_err(|_| {
                error(
                    "SUBSCRIPTION_CONNECTION",
                    "Could not renew ChatGPT access. Check your connection and try again.",
                )
            })?;
        if response.status().as_u16() == 400 || response.status().as_u16() == 401 {
            self.tokens = None;
            return Err(auth_error());
        }
        self.tokens = Some(read_tokens(&json(response)?, Some(tokens))?);
        Ok(())
    }
    /// Always forget locally, even when the caller cannot confirm remote revocation.
    pub fn revoke(&mut self) -> AppResult<()> {
        let Some(tokens) = self.tokens.take() else {
            return Ok(());
        };
        let work = || -> AppResult<()> {
            let client = client()?;
            let config = discovery(&client)?;
            let url = auth_endpoint(&config, "revocation_endpoint")?;
            let response = client
                .post(url)
                .form(&[
                    ("token", tokens.refresh_token.as_str()),
                    ("token_type_hint", "refresh_token"),
                    ("client_id", self.client_id.as_str()),
                ])
                .send()
                .map_err(|_| auth_error())?;
            if response.status().as_u16() != 200 {
                return Err(auth_error());
            }
            Ok(())
        };
        work().map_err(|_| error("REVOCATION_UNCONFIRMED", "Signed out on this computer. Remote revocation was not confirmed; disconnect FrameIQ in ChatGPT Settings."))
    }
}
fn read_tokens(value: &Value, previous: Option<&Tokens>) -> AppResult<Tokens> {
    let field = |name: &str| {
        value[name]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 32768)
            .map(str::to_owned)
    };
    let scopes = value["scope"]
        .as_str()
        .map(|s| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .or_else(|| previous.map(|t| t.scopes.clone()))
        .ok_or_else(auth_error)?;
    if !["resource.invoke", "chatgpt.tokens.use.direct"]
        .iter()
        .all(|required| scopes.iter().any(|s| s == required))
    {
        return Err(error(
            "SUBSCRIPTION_PERMISSION",
            "ChatGPT plan usage was not authorized. Continue with ChatGPT and allow plan usage.",
        ));
    }
    if !value["token_type"]
        .as_str()
        .is_some_and(|t| t.eq_ignore_ascii_case("bearer"))
    {
        return Err(auth_error());
    }
    let access_token = field("access_token").ok_or_else(auth_error)?;
    super::cloud::validate_key(&access_token).map_err(|_| auth_error())?;
    let expires_in = value["expires_in"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(auth_error)?;
    Ok(Tokens {
        access_token,
        refresh_token: field("refresh_token")
            .or_else(|| previous.map(|t| t.refresh_token.clone()))
            .ok_or_else(auth_error)?,
        id_token: field("id_token")
            .or_else(|| previous.map(|t| t.id_token.clone()))
            .ok_or_else(auth_error)?,
        expires_at: now().saturating_add(expires_in),
        scopes,
    })
}

pub struct SignIn {
    listener: TcpListener,
    redirect: String,
    state: String,
    nonce: String,
    verifier: String,
    previous: Option<Account>,
    initial_client_id: String,
    started: Instant,
}
impl SignIn {
    pub fn new(previous: Option<Account>) -> AppResult<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| {
            error(
                "OAUTH_CALLBACK",
                "Could not open the local sign-in callback. Try again.",
            )
        })?;
        listener.set_nonblocking(true).map_err(|_| auth_error())?;
        let redirect = format!(
            "http://127.0.0.1:{}/auth/callback",
            listener.local_addr().map_err(|_| auth_error())?.port()
        );
        let initial_client_id = previous
            .as_ref()
            .map(|a| a.client_id.clone())
            .or_else(|| {
                std::env::var("FRAMEIQ_OPENAI_CLIENT_ID")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_else(|| "dynamic_agent_client".into());
        Ok(Self {
            listener,
            redirect,
            state: random(),
            nonce: random(),
            verifier: random(),
            previous,
            initial_client_id,
            started: Instant::now(),
        })
    }
    pub fn authorization_url(&self, host_id: &str) -> String {
        let mut url =
            Url::parse("https://auth.openai.com/api/accounts/authorize").expect("fixed URL");
        let mut query = url.query_pairs_mut();
        query.extend_pairs([
            ("client_id", self.initial_client_id.as_str()),
            ("ext_agent_host_id", host_id),
            ("response_type", "code"),
            ("redirect_uri", self.redirect.as_str()),
            ("scope", SCOPE),
            ("resource", RESOURCE),
            ("state", self.state.as_str()),
            ("nonce", self.nonce.as_str()),
            ("code_challenge_method", "S256"),
            (
                "code_challenge",
                URL_SAFE_NO_PAD
                    .encode(Sha256::digest(self.verifier.as_bytes()))
                    .as_str(),
            ),
        ]);
        if self.initial_client_id == "dynamic_agent_client" {
            query.append_pair("agent_name_hint", "FrameIQ");
        }
        if let Some(account) = &self.previous {
            if let Some(tokens) = &account.tokens {
                query.append_pair("id_token_hint", &tokens.id_token);
            }
            if let Some(email) = &account.email {
                query.append_pair("login_hint", email);
            }
        }
        drop(query);
        url.into()
    }
    fn callback(&self, target: &str) -> AppResult<(String, String)> {
        if !target.starts_with("/auth/callback?") {
            return Err(auth_error());
        }
        let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| auth_error())?;
        let pairs = url.query_pairs().collect::<Vec<_>>();
        let get = |name: &str| -> AppResult<Option<String>> {
            let values = pairs.iter().filter(|(k, _)| k == name).collect::<Vec<_>>();
            if values.len() > 1 {
                return Err(auth_error());
            }
            Ok(values.first().map(|(_, v)| v.to_string()))
        };
        if get("state")?.as_deref() != Some(&self.state) {
            return Err(auth_error());
        }
        if get("error")?.is_some() {
            return Err(error(
                "OAUTH_DECLINED",
                "ChatGPT sign-in was declined. Continue with ChatGPT when you are ready.",
            ));
        }
        let code = get("code")?
            .filter(|s| !s.is_empty())
            .ok_or_else(auth_error)?;
        let issued = get("client_id")?;
        let client_id = if self.initial_client_id == "dynamic_agent_client" {
            issued
                .filter(|s| s.starts_with("oaiapp_") && s.len() <= 256)
                .ok_or_else(auth_error)?
        } else {
            if issued
                .as_ref()
                .is_some_and(|id| id != &self.initial_client_id)
            {
                return Err(auth_error());
            }
            self.initial_client_id.clone()
        };
        Ok((code, client_id))
    }
    pub fn complete(self, cancelled: &AtomicBool) -> AppResult<Account> {
        loop {
            if cancelled.load(Ordering::SeqCst) {
                return Err(error("CANCELLED", "ChatGPT sign-in cancelled."));
            }
            if self.started.elapsed() > Duration::from_secs(300) {
                return Err(error(
                    "OAUTH_EXPIRED",
                    "ChatGPT sign-in expired. Continue with ChatGPT again.",
                ));
            }
            match self.listener.accept() {
                Ok((mut stream, _)) => {
                    let target = read_target(&mut stream)?;
                    if !target.starts_with("/auth/callback?") {
                        reply(
                            &mut stream,
                            "404 Not Found",
                            "Return to FrameIQ to sign in.",
                        );
                        continue;
                    }
                    let callback = self.callback(&target);
                    reply(&mut stream, "200 OK", "Return to FrameIQ to check your ChatGPT connection. You can close this window.");
                    let (code, client_id) = callback?;
                    if cancelled.load(Ordering::SeqCst) {
                        return Err(error("CANCELLED", "ChatGPT sign-in cancelled."));
                    }
                    return self.exchange(&code, &client_id);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(25))
                }
                Err(_) => return Err(auth_error()),
            }
        }
    }
    fn exchange(&self, code: &str, client_id: &str) -> AppResult<Account> {
        let client = client()?;
        let value = json(
            client
                .post(TOKEN_URL)
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("client_id", client_id),
                    ("code", code),
                    ("code_verifier", &self.verifier),
                    ("redirect_uri", &self.redirect),
                    ("resource", RESOURCE),
                ])
                .send()
                .map_err(|_| auth_error())?,
        )?;
        let tokens = read_tokens(&value, None)?;
        let config = discovery(&client)?;
        let jwks: JwkSet = serde_json::from_value(json(
            client
                .get(auth_endpoint(&config, "jwks_uri")?)
                .send()
                .map_err(|_| auth_error())?,
        )?)
        .map_err(|_| auth_error())?;
        let claims = verify_identity(&tokens.id_token, client_id, &self.nonce, &jwks)?;
        if self
            .previous
            .as_ref()
            .is_some_and(|a| a.subject != claims.sub)
        {
            return Err(auth_error());
        }
        Ok(Account {
            id: self
                .previous
                .as_ref()
                .map(|a| a.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            client_id: client_id.into(),
            subject: claims.sub,
            email: claims.email,
            tokens: Some(tokens),
        })
    }
}
#[derive(Deserialize)]
struct Claims {
    sub: String,
    nonce: String,
    iat: u64,
    email: Option<String>,
}
fn verify_identity(token: &str, client_id: &str, nonce: &str, keys: &JwkSet) -> AppResult<Claims> {
    let header = decode_header(token).map_err(|_| auth_error())?;
    if !matches!(header.alg, Algorithm::RS256 | Algorithm::ES256) {
        return Err(auth_error());
    }
    let key = keys
        .find(header.kid.as_deref().ok_or_else(auth_error)?)
        .ok_or_else(auth_error)?;
    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    validation.set_required_spec_claims(&["iss", "aud", "exp", "sub", "iat"]);
    validation.leeway = 5;
    validation.validate_nbf = true;
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_jwk(key).map_err(|_| auth_error())?,
        &validation,
    )
    .map_err(|_| auth_error())?
    .claims;
    if claims.nonce != nonce || claims.sub.is_empty() || claims.iat > now() + 5 {
        return Err(auth_error());
    }
    Ok(claims)
}
fn read_target(stream: &mut TcpStream) -> AppResult<String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|_| auth_error())?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 1024];
    while bytes.len() < 8192 {
        let n = stream.read(&mut buffer).map_err(|_| auth_error())?;
        if n == 0 {
            return Err(auth_error());
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            let request = std::str::from_utf8(&bytes).map_err(|_| auth_error())?;
            let mut parts = request
                .lines()
                .next()
                .ok_or_else(auth_error)?
                .split_whitespace();
            if parts.next() != Some("GET") {
                return Err(auth_error());
            }
            return parts.next().map(str::to_owned).ok_or_else(auth_error);
        }
    }
    Err(auth_error())
}
fn reply(stream: &mut TcpStream, status: &str, message: &str) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Security-Policy: default-src 'none'\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{message}", message.len());
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use ring::{
        rand::SystemRandom,
        signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING},
    };
    use serde_json::json;

    fn signed_claims(claims: Value) -> (String, JwkSet) {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng)
            .unwrap();
        let public = pair.public_key().as_ref();
        let jwks = serde_json::from_value(json!({"keys":[{"kty":"EC","crv":"P-256","kid":"test","alg":"ES256","use":"sig","x":URL_SAFE_NO_PAD.encode(&public[1..33]),"y":URL_SAFE_NO_PAD.encode(&public[33..65])}]})).unwrap();
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some("test".into());
        (
            encode(&header, &claims, &EncodingKey::from_ec_der(pkcs8.as_ref())).unwrap(),
            jwks,
        )
    }
    #[test]
    fn identity_requires_signature_issuer_audience_expiry_subject_and_nonce() {
        let claims = json!({"iss":ISSUER,"aud":["oaiapp_test"],"sub":"verified-user","iat":now(),"exp":now()+600,"nonce":"expected"});
        let (token, keys) = signed_claims(claims.clone());
        assert_eq!(
            verify_identity(&token, "oaiapp_test", "expected", &keys)
                .unwrap()
                .sub,
            "verified-user"
        );
        assert!(verify_identity(&token, "oaiapp_other", "expected", &keys).is_err());
        assert!(verify_identity(&token, "oaiapp_test", "wrong-nonce", &keys).is_err());
        let (_, other_keys) = signed_claims(claims.clone());
        assert!(verify_identity(&token, "oaiapp_test", "expected", &other_keys).is_err());
        for (field, value) in [
            ("iss", json!("https://attacker.invalid")),
            ("exp", json!(now() - 60)),
            ("iat", json!(now() + 60)),
            ("sub", json!("")),
        ] {
            let mut invalid = claims.clone();
            invalid[field] = value;
            let (token, keys) = signed_claims(invalid);
            assert!(verify_identity(&token, "oaiapp_test", "expected", &keys).is_err());
        }
    }
    #[test]
    fn authorization_uses_loopback_pkce_stable_host_and_subscription_scopes() {
        let attempt = SignIn::new(None).unwrap();
        let url = Url::parse(&attempt.authorization_url("urn:uuid:stable-host")).unwrap();
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(url.host_str(), Some("auth.openai.com"));
        assert_eq!(pairs["agent_name_hint"], "FrameIQ");
        assert_eq!(pairs["ext_agent_host_id"], "urn:uuid:stable-host");
        assert_eq!(pairs["redirect_uri"], attempt.redirect);
        assert!(attempt.redirect.starts_with("http://127.0.0.1:"));
        assert_eq!(
            pairs["code_challenge"],
            URL_SAFE_NO_PAD.encode(Sha256::digest(attempt.verifier.as_bytes()))
        );
        assert!(pairs["scope"].contains("chatgpt.tokens.use.direct"));
        assert!(!url.as_str().contains(&attempt.verifier));
    }
    #[test]
    fn callback_rejects_state_injection_missing_client_and_client_substitution() {
        let attempt = SignIn::new(None).unwrap();
        let valid = format!(
            "/auth/callback?state={}&code=test-code&client_id=oaiapp_issued",
            attempt.state
        );
        assert_eq!(
            attempt.callback(&valid).unwrap(),
            ("test-code".into(), "oaiapp_issued".into())
        );
        for invalid in [
            valid.replace(&attempt.state, "wrong"),
            format!("{valid}&state=duplicate"),
            valid.replace("&client_id=oaiapp_issued", ""),
            format!("/auth/callback?state={}&error=access_denied", attempt.state),
        ] {
            assert!(attempt.callback(&invalid).is_err());
        }
        let previous = Account {
            id: uuid::Uuid::new_v4().to_string(),
            client_id: "oaiapp_saved".into(),
            subject: "user".into(),
            email: None,
            tokens: None,
        };
        let returning = SignIn::new(Some(previous)).unwrap();
        let valid = format!("/auth/callback?state={}&code=returning", returning.state);
        assert_eq!(returning.callback(&valid).unwrap().1, "oaiapp_saved");
        assert!(returning
            .callback(&format!("{valid}&client_id=oaiapp_other"))
            .is_err());
    }
    #[test]
    fn refresh_keeps_grants_and_replaces_rotating_credentials_together() {
        let value = json!({"token_type":"Bearer","access_token":"old-access","refresh_token":"old-refresh","id_token":"id-token","scope":"resource.invoke chatgpt.tokens.use.direct offline_access","expires_in":3600});
        let previous = read_tokens(&value, None).unwrap();
        let next = read_tokens(&json!({"token_type":"Bearer","access_token":"new-access","refresh_token":"new-refresh","expires_in":1800}),Some(&previous)).unwrap();
        assert_eq!(next.refresh_token, "new-refresh");
        assert_eq!(next.scopes, previous.scopes);
        let mut denied = value;
        denied["scope"] = json!("openid profile email");
        assert_eq!(
            read_tokens(&denied, None).err().unwrap().code,
            "SUBSCRIPTION_PERMISSION"
        );
    }
    #[test]
    fn cancelled_signin_does_not_exchange_credentials() {
        let result = SignIn::new(None).unwrap().complete(&AtomicBool::new(true));
        assert_eq!(result.err().unwrap().code, "CANCELLED");
    }
}
