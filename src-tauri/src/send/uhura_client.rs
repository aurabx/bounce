//! WADO-RS client for fetching study bytes from Uhura.
//!
//! Aura mints a short-lived, study-scoped JWT and includes it in each pending
//! send job (see [`crate::query::models::WadoSource`]). This client uses that
//! JWT to call Uhura's `/dicomweb/v3/raw/studies/{uid}` endpoint, parses the
//! `multipart/related; type=application/dicom` response, and yields each
//! instance as an in-memory DICOM file object.
//!
//! The implementation only needs to handle Uhura-shaped responses: each part
//! is a complete DICOM file (preamble + magic + meta + dataset) so we can
//! defer to `dicom-rs` for parsing each part.

use crate::log_info;
use dicom::object::{FileDicomObject, InMemDicomObject};
use std::io::Cursor;

/// HTTP client wrapper for Uhura's WADO-RS surface.
#[derive(Clone, Debug)]
pub struct UhuraClient {
    client: reqwest::Client,
}

impl UhuraClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    #[allow(dead_code)]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetch every instance of a study from Uhura.
    ///
    /// `base_url` is the absolute Uhura WADO-RS root (e.g. `https://uhura/dicomweb/v3/raw`);
    /// `study_path` is the per-study suffix (`/studies/{uid}`); `jwt` is the
    /// study-scoped bearer token Aura minted for this send.
    pub async fn fetch_study(
        &self,
        base_url: &str,
        study_path: &str,
        jwt: &str,
    ) -> anyhow::Result<Vec<FileDicomObject<InMemDicomObject>>> {
        let url = format!("{}{}", base_url.trim_end_matches('/'), study_path);

        log_info!("WADO-RS: GET {}", url);

        let response = self
            .client
            .get(&url)
            .header(
                "Accept",
                "multipart/related; type=\"application/dicom\"",
            )
            .header("Authorization", format!("Bearer {}", jwt))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "WADO-RS GET {} failed: HTTP {} - {}",
                url,
                status,
                body,
            ));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("WADO-RS response missing Content-Type"))?;

        let boundary = extract_boundary(&content_type).ok_or_else(|| {
            anyhow::anyhow!(
                "WADO-RS response Content-Type missing boundary parameter: {}",
                content_type,
            )
        })?;

        let body = response.bytes().await?;

        let parts = split_multipart_parts(&body, &boundary)?;

        log_info!("WADO-RS: received {} multipart parts from {}", parts.len(), url);

        let mut instances = Vec::with_capacity(parts.len());
        for (idx, part) in parts.into_iter().enumerate() {
            let cursor = Cursor::new(part);
            let obj = FileDicomObject::<InMemDicomObject>::from_reader(cursor).map_err(|e| {
                anyhow::anyhow!("Failed to parse DICOM part {} from WADO-RS response: {}", idx, e)
            })?;
            instances.push(obj);
        }

        Ok(instances)
    }
}

impl Default for UhuraClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the `boundary` parameter from a multipart `Content-Type` header.
///
/// Handles both quoted and unquoted forms:
///   - `multipart/related; boundary="abc123"`
///   - `multipart/related; type=application/dicom; boundary=abc123`
pub(crate) fn extract_boundary(content_type: &str) -> Option<String> {
    for raw_param in content_type.split(';').skip(1) {
        let param = raw_param.trim();
        if let Some(value) = param.strip_prefix("boundary=") {
            let trimmed = value.trim();
            let unquoted = trimmed
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(trimmed);
            if !unquoted.is_empty() {
                return Some(unquoted.to_string());
            }
        }
    }
    None
}

/// Split a `multipart/related` body into the body bytes of each part.
///
/// The protocol uses `--{boundary}` as a separator and `--{boundary}--` to
/// terminate the message. Each part is `headers \r\n\r\n body` where `body`
/// runs up to (but not including) the CRLF preceding the next boundary.
///
/// Per RFC 2046, boundaries appear at the start of a line. The trailing CRLF
/// before each boundary is part of the boundary delimiter, not the part body
/// — we trim it so the returned slice is a valid DICOM file blob.
pub(crate) fn split_multipart_parts(
    body: &[u8],
    boundary: &str,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let dash_boundary = format!("--{}", boundary);
    let dash_boundary_bytes = dash_boundary.as_bytes();

    // Find the first occurrence of the boundary (the preamble before it is
    // discardable per RFC 2046).
    let first = find_subslice(body, dash_boundary_bytes)
        .ok_or_else(|| anyhow::anyhow!("multipart body missing initial boundary"))?;

    let mut cursor = first + dash_boundary_bytes.len();
    let mut parts = Vec::new();

    loop {
        // Two characters after a boundary: either CRLF (another part follows)
        // or "--" (this is the closing boundary, end of message).
        if cursor + 2 > body.len() {
            return Err(anyhow::anyhow!(
                "multipart body truncated immediately after a boundary"
            ));
        }
        if &body[cursor..cursor + 2] == b"--" {
            // Closing boundary reached; we're done.
            return Ok(parts);
        }

        // Skip the CRLF that follows a non-final boundary.
        let header_start = if &body[cursor..cursor + 2] == b"\r\n" {
            cursor + 2
        } else {
            // Be lenient: some senders use bare LF.
            cursor + 1
        };

        // Find end of headers (CRLF CRLF or LF LF) within this part.
        let header_end = find_subslice(&body[header_start..], b"\r\n\r\n")
            .map(|i| header_start + i + 4)
            .or_else(|| find_subslice(&body[header_start..], b"\n\n").map(|i| header_start + i + 2))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "multipart part missing header/body separator at offset {}",
                    header_start,
                )
            })?;

        // Find the next boundary marker.
        let next_boundary = find_subslice(&body[header_end..], dash_boundary_bytes)
            .map(|i| header_end + i)
            .ok_or_else(|| anyhow::anyhow!("multipart part missing terminating boundary"))?;

        // The CRLF before the boundary is part of the delimiter, not the body.
        let mut body_end = next_boundary;
        if body_end >= 2 && &body[body_end - 2..body_end] == b"\r\n" {
            body_end -= 2;
        } else if body_end >= 1 && body[body_end - 1] == b'\n' {
            body_end -= 1;
        }

        if body_end < header_end {
            return Err(anyhow::anyhow!("multipart part has negative-length body"));
        }

        parts.push(body[header_end..body_end].to_vec());
        cursor = next_boundary + dash_boundary_bytes.len();
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
