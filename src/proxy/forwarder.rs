use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;

/// Forward a request to the real LLM API and return the raw response.
pub async fn forward_request(
    client: &Client,
    target_url: &str,
    method: &str,
    headers: &HashMap<String, String>,
    body: bytes::Bytes,
) -> Result<reqwest::Response> {
    let mut req_builder = match method {
        "POST" => client.post(target_url),
        "GET" => client.get(target_url),
        "PUT" => client.put(target_url),
        "DELETE" => client.delete(target_url),
        "PATCH" => client.patch(target_url),
        _ => client.post(target_url),
    };

    // Forward relevant headers
    for (key, value) in headers {
        let key_lower = key.to_lowercase();
        // Skip hop-by-hop headers
        if key_lower == "host" || key_lower == "connection" || key_lower == "content-length" {
            continue;
        }
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    // Send the request body
    if !body.is_empty() {
        req_builder = req_builder.body(body);
    }

    let response = req_builder.send().await?;

    Ok(response)
}

/// Build the target URL from provider base URL and the original request path.
pub fn build_target_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{}/{}", base, path)
}

/// Check if a response is an SSE stream.
pub fn is_sse_response(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/event-stream"))
        .unwrap_or(false)
}
