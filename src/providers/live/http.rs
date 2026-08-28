use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

fn client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(2)
            .redirect(reqwest::redirect::Policy::limited(3))
            .user_agent(concat!("headroom/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("valid HTTP client configuration")
    })
}

fn read_body(response: reqwest::blocking::Response) -> Result<String> {
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES)
    {
        bail!("HTTP response exceeded 1 MiB");
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("could not read HTTP response")?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        bail!("HTTP response exceeded 1 MiB");
    }
    String::from_utf8(bytes).context("response was not UTF-8")
}

pub fn request(
    url: &str,
    headers: &[(&str, String)],
    body: Option<String>,
) -> Result<HttpResponse> {
    let mut request = if body.is_some() {
        client().post(url)
    } else {
        client().get(url)
    };
    for (name, value) in headers {
        let name =
            HeaderName::from_bytes(name.as_bytes()).context("invalid request header name")?;
        let value = HeaderValue::from_str(value).context("invalid request header value")?;
        request = request.header(name, value);
    }
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().context("HTTP request failed")?;
    let status = response.status().as_u16();
    let response_headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect();
    let body = read_body(response)?;
    Ok(HttpResponse {
        status,
        headers: response_headers,
        body,
    })
}

pub fn json(url: &str, headers: &[(&str, String)], body: Value) -> Result<Value> {
    let response = request(url, headers, Some(body.to_string()))?;
    if !(200..300).contains(&response.status) {
        bail!("HTTP {}", response.status);
    }
    serde_json::from_str(&response.body).context("response was not JSON")
}

pub fn json_number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.parse().ok(),
        _ => None,
    }
}

pub fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
