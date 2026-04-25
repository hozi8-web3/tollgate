use anyhow::Result;
use axum::body::Body;
use bytes::Bytes;
use futures_util::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

/// Stream an SSE response back to the client while capturing chunks for token extraction.
/// Returns the full accumulated body bytes and a streaming Body for the client.
pub async fn stream_response(
    response: reqwest::Response,
) -> Result<(Body, Vec<Bytes>)> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(32);
    let chunks: Vec<Bytes> = Vec::new();

    let mut stream = response.bytes_stream();
    let chunks_clone = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let chunks_ref = chunks_clone.clone();

    tokio::spawn(async move {
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    chunks_ref.lock().await.push(chunk.clone());
                    if tx.send(Ok(chunk)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))).await;
                    break;
                }
            }
        }
    });

    let body_stream = ReceiverStream::new(rx);
    let body = Body::from_stream(body_stream);

    // Wait a moment for chunks to be collected — the actual full body
    // will be reconstructed from the collected chunks after streaming completes
    // We return the chunks collector for the caller to await
    Ok((body, chunks))
}

/// Parse SSE stream data to extract the final usage chunk.
/// Many providers send usage data in the final SSE `data: [DONE]` or a `data: {...}` with usage.
pub fn extract_usage_from_sse_chunks(chunks: &[Bytes]) -> Option<serde_json::Value> {
    let full_text = chunks.iter()
        .filter_map(|c| std::str::from_utf8(c).ok())
        .collect::<String>();

    // Look for the last SSE data line that contains usage information
    for line in full_text.lines().rev() {
        let line = line.trim();
        if line.starts_with("data: ") {
            let data = &line[6..];
            if data == "[DONE]" {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                // Check if this chunk has usage data
                if parsed.get("usage").is_some() {
                    return Some(parsed);
                }
            }
        }
    }

    None
}
