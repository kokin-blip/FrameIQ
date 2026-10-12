//! Explicit metadata-only provider connection tests. No media or generation requests.
use frameiq_contracts::*;
use reqwest::{blocking::Client, StatusCode};
use serde_json::Value;
use std::{io::Read, time::Duration};

pub fn validate_key(key: &str) -> AppResult<()> {
    if key.is_empty() || key.len() > 8192 || !key.bytes().all(|c| c.is_ascii_graphic()) {
        return Err(AppError::new(
            "INVALID_API_KEY",
            "Enter an API key without spaces or line breaks.",
        ));
    }
    Ok(())
}
pub(super) fn status_error(status: StatusCode) -> AppError {
    let (code, message) = match status.as_u16() {
        401 | 403 => (
            "AUTH_REQUIRED",
            "The provider rejected this key. Check its permissions or replace it.",
        ),
        400 | 404 => ("MODEL_INCOMPATIBLE", "The provider rejected the request. Check the model ID and its image/structured-output support."),
        429 => (
            "RATE_LIMITED",
            "The provider rate limit was reached. Try again later.",
        ),
        _ => (
            "CLOUD_CONNECTION",
            "The provider could not complete the request. Try again later.",
        ),
    };
    AppError::new(code, message)
}
fn parse_models(provider: CloudProviderV1, value: &Value) -> AppResult<CloudConnectionV1> {
    let key = match provider {
        CloudProviderV1::Openai => "data",
        CloudProviderV1::Gemini => "models",
    };
    let models = value[key].as_array().ok_or_else(|| {
        AppError::new(
            "CLOUD_RESPONSE",
            "The provider returned an unsupported model list.",
        )
    })?;
    Ok(CloudConnectionV1 {
        provider,
        model_count: models.len().min(u32::MAX as usize) as u32,
        models: models
            .iter()
            .filter_map(|m| {
                m[if matches!(provider, CloudProviderV1::Openai) {
                    "id"
                } else {
                    "name"
                }]
                .as_str()
            })
            .filter(|id| id.len() <= 128)
            .take(256)
            .map(str::to_owned)
            .collect(),
        more_available: value
            .get("nextPageToken")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty())
            || value["has_more"] == true,
    })
}
pub(super) fn gemini_key(key: &str) -> reqwest::header::HeaderValue {
    // Callers validate visible ASCII before constructing a request.
    let mut value = reqwest::header::HeaderValue::from_str(key).expect("validated API key");
    value.set_sensitive(true);
    value
}
fn model_request(
    client: &Client,
    provider: CloudProviderV1,
    key: &str,
) -> reqwest::blocking::RequestBuilder {
    match provider {
        CloudProviderV1::Openai => client
            .get("https://api.openai.com/v1/models")
            .bearer_auth(key),
        CloudProviderV1::Gemini => client
            .get("https://generativelanguage.googleapis.com/v1beta/models?pageSize=100")
            .header("x-goog-api-key", gemini_key(key)),
    }
}
pub fn test_connection(provider: CloudProviderV1, key: &str) -> AppResult<CloudConnectionV1> {
    validate_key(key)?;
    let client = Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| {
            AppError::new(
                "CLOUD_CONNECTION",
                "Could not initialize a secure connection.",
            )
        })?;
    let request = model_request(&client, provider, key);
    // Never surface provider bodies or transport diagnostics: either can echo credentials.
    let response = request.send().map_err(|_| {
        AppError::new(
            "CLOUD_CONNECTION",
            "Could not reach the provider. Check your connection and try again.",
        )
    })?;
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            AppError::new(
                "CLOUD_CONNECTION",
                "The provider response was interrupted. Try again.",
            )
        })?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(AppError::new(
            "CLOUD_RESPONSE",
            "The provider response exceeded the size limit.",
        ));
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::new(
            "CLOUD_RESPONSE",
            "The provider returned an invalid response.",
        )
    })?;
    parse_models(provider, &value)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn connection_tests_send_metadata_only_to_fixed_hosts() {
        let client = Client::new();
        for (provider, host, header, value) in [
            (
                CloudProviderV1::Openai,
                "api.openai.com",
                "authorization",
                "Bearer fake-key",
            ),
            (
                CloudProviderV1::Gemini,
                "generativelanguage.googleapis.com",
                "x-goog-api-key",
                "fake-key",
            ),
        ] {
            let request = model_request(&client, provider, "fake-key")
                .build()
                .unwrap();
            assert_eq!(request.method(), reqwest::Method::GET);
            assert_eq!(request.url().scheme(), "https");
            assert_eq!(request.url().host_str(), Some(host));
            assert_eq!(request.headers()[header], value);
            assert!(request.headers()[header].is_sensitive());
            assert!(!request.url().as_str().contains("fake-key"));
            assert!(request.body().is_none());
        }
    }
    #[test]
    fn key_validation_rejects_header_injection() {
        for key in ["", "secret\r\nOther: value", "secret token", "é"] {
            assert!(validate_key(key).is_err());
        }
        assert!(validate_key("test-key_123").is_ok());
        assert!(validate_key(&"x".repeat(8193)).is_err());
    }
    #[test]
    fn model_lists_preserve_provider_and_partial_page() {
        let openai = parse_models(
            CloudProviderV1::Openai,
            &json!({"data":[{"id":"a"},{"id":"b"}]}),
        )
        .unwrap();
        assert_eq!(openai.model_count, 2);
        assert!(!openai.more_available);
        let gemini = parse_models(
            CloudProviderV1::Gemini,
            &json!({"models":[],"nextPageToken":"next"}),
        )
        .unwrap();
        assert_eq!(gemini.model_count, 0);
        assert!(gemini.more_available);
        assert!(parse_models(CloudProviderV1::Openai, &json!({"error":"sensitive-body"})).is_err());
    }
    #[test]
    fn errors_are_safe_actionable_codes() {
        assert_eq!(status_error(StatusCode::FORBIDDEN).code, "AUTH_REQUIRED");
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS).code,
            "RATE_LIMITED"
        );
        assert_eq!(
            status_error(StatusCode::BAD_GATEWAY).code,
            "CLOUD_CONNECTION"
        );
    }
}
