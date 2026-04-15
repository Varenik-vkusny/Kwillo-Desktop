use std::path::PathBuf;

pub const MAX_RETRIES: u32 = 3;
pub const RETRY_DELAY_SECS: u64 = 30;

/// Upload an MP3 file to the given URL via multipart POST.
/// Retries up to MAX_RETRIES times with RETRY_DELAY_SECS delay between attempts.
/// Deletes the local file on success.
pub async fn upload_and_delete(path: PathBuf, upload_url: &str) -> Result<(), String> {
    let mut last_error = String::new();

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
        }

        match try_upload(&path, upload_url).await {
            Ok(_) => {
                std::fs::remove_file(&path)
                    .map_err(|e| format!("Upload succeeded but failed to delete file: {e}"))?;
                return Ok(());
            }
            Err(e) => {
                last_error = e.clone();
                eprintln!(
                    "[Uploader] Attempt {}/{} failed: {}",
                    attempt + 1,
                    MAX_RETRIES,
                    e
                );
            }
        }
    }

    Err(format!(
        "Upload failed after {} attempts. Last error: {}. File kept at {:?}",
        MAX_RETRIES, last_error, path
    ))
}

async fn try_upload(path: &PathBuf, upload_url: &str) -> Result<(), String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Cannot read file: {e}"))?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.mp3")
        .to_string();

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename)
        .mime_str("audio/mpeg")
        .map_err(|e| format!("MIME error: {e}"))?;

    let form = reqwest::multipart::Form::new().part("audio", part);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Cannot build HTTP client: {e}"))?;

    let response = client
        .post(upload_url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if response.status().is_success() {
        match response.json::<serde_json::Value>().await {
            Ok(body) => {
                println!("[Uploader] Success: {body}");
                Ok(())
            }
            Err(_) => {
                println!("[Uploader] Success (no body)");
                Ok(())
            }
        }
    } else {
        Err(format!("Server returned HTTP {}", response.status()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_max_retries_is_three() {
        assert_eq!(super::MAX_RETRIES, 3);
    }

    #[test]
    fn test_retry_delay_is_thirty_seconds() {
        assert_eq!(super::RETRY_DELAY_SECS, 30);
    }
}
