use frameiq_contracts::*;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{io::Read, time::Duration};
fn error(e: impl std::fmt::Display) -> AppError {
    AppError::new("LOCAL_RUNTIME", e.to_string())
}
fn authorize(
    request: reqwest::blocking::RequestBuilder,
    token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    if let Some(token) = token {
        request.bearer_auth(token)
    } else {
        request
    }
}
fn client() -> AppResult<Client> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(error)
}
fn endpoint(runtime: &str, port: u16) -> AppResult<String> {
    if !["ollama", "lm_studio"].contains(&runtime) || port == 0 {
        return Err(error(
            "Choose Ollama or LM Studio and a valid loopback port.",
        ));
    }
    Ok(format!("http://127.0.0.1:{port}"))
}
fn body(response: reqwest::blocking::Response) -> AppResult<Value> {
    let response = response.error_for_status().map_err(error)?;
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(error("Runtime response exceeds limit."));
    }
    serde_json::from_slice(&bytes).map_err(error)
}
pub fn discover(runtime: &str, port: u16, token: Option<&str>) -> AppResult<LocalRuntimeReportV1> {
    let client = client()?;
    let base = endpoint(runtime, port)?;
    let mut models = Vec::new();
    if runtime == "ollama" {
        let mut request = client.get(format!("{base}/api/tags"));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let data = body(request.send().map_err(error)?)?;
        for item in data["models"]
            .as_array()
            .ok_or_else(|| error("Unsupported model list response."))?
            .iter()
            .take(64)
        {
            let Some(id) = item["name"].as_str() else {
                continue;
            };
            let info = body(
                authorize(client.post(format!("{base}/api/show")), token)
                    .json(&json!({"model":id}))
                    .send()
                    .map_err(error)?,
            )?;
            let local = info
                .get("remote_host")
                .is_none_or(|v| v.is_null() || v == "")
                && info
                    .get("remote_model")
                    .is_none_or(|v| v.is_null() || v == "")
                && !id.ends_with(":cloud")
                && !id.ends_with("-cloud");
            let vision = info["capabilities"]
                .as_array()
                .is_some_and(|v| v.iter().any(|c| c == "vision"));
            models.push(LocalRuntimeModelV1 {
                id: id.into(),
                vision,
                local,
            });
        }
    } else {
        let mut request = client.get(format!("{base}/api/v1/models"));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let data = body(request.send().map_err(error)?)?;
        for item in data["models"]
            .as_array()
            .ok_or_else(|| error("LM Studio v1 model listing is required."))?
            .iter()
            .take(64)
        {
            let Some(id) = item["key"].as_str().or(item["id"].as_str()) else {
                continue;
            };
            models.push(LocalRuntimeModelV1 {
                id: id.into(),
                vision: item["capabilities"]["vision"] == true,
                local: true,
            });
        }
    }
    Ok(LocalRuntimeReportV1 {
        runtime: runtime.into(),
        models,
    })
}
pub fn probe(runtime: &str, port: u16, model: &str, token: Option<&str>) -> AppResult<f64> {
    let report = discover(runtime, port, token)?;
    if !report
        .models
        .iter()
        .any(|m| m.id == model && m.vision && m.local)
    {
        return Err(error("Select a locally hosted model that supports images."));
    }
    // Bundled synthetic 1×1 PNG only; no project image is transmitted by compatibility probes.
    let image="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aNGUAAAAASUVORK5CYII=";
    let client = client()?;
    let base = endpoint(runtime, port)?;
    let started = std::time::Instant::now();
    let response = if runtime == "ollama" {
        authorize(client.post(format!("{base}/api/generate")),token).json(&json!({"model":model,"prompt":"Describe this synthetic pixel briefly.","images":[image],"stream":false,"options":{"num_predict":32}})).send().map_err(error)?
    } else {
        let mut r=client.post(format!("{base}/v1/chat/completions")).json(&json!({"model":model,"messages":[{"role":"user","content":[{"type":"text","text":"Describe this synthetic pixel briefly."},{"type":"image_url","image_url":{"url":format!("data:image/png;base64,{image}")}}]}],"max_tokens":32,"stream":false}));
        if let Some(token) = token {
            r = r.bearer_auth(token);
        }
        r.send().map_err(error)?
    };
    let value = body(response)?;
    if runtime == "ollama" {
        if !value["response"].is_string() {
            return Err(error("Runtime did not return a vision response."));
        }
    } else if !value["choices"][0]["message"]["content"].is_string() {
        return Err(error("Runtime did not return a vision response."));
    }
    Ok(started.elapsed().as_secs_f64() * 1000.)
}
