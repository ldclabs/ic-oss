use futures::StreamExt;
use ic_oss::bucket::{Client, Progress};
use ic_oss_types::{file::*, format_error};
use jiff::{Timestamp, ZonedDateTime};
use sha3::{Digest, Sha3_256};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::{time, time::Duration};

// Six chunks stay below the canister's per-call response limit.
const BATCH_CHUNKS: u32 = 6;

pub async fn upload_file(cli: &Client, parent: u32, file: &str, retry: u8) -> Result<(), String> {
    let file_path = Path::new(file);
    let metadata = std::fs::metadata(file_path).map_err(format_error)?;
    if !metadata.is_file() {
        return Err(format!("not a file: {:?}", file));
    }

    let file_size = metadata.len();
    let content_type = infer2::get_from_path(file_path)
        .map_err(format_error)?
        .map(|f| f.mime_type());

    let content_type = if let Some(content_type) = content_type {
        content_type
    } else {
        mime_db::lookup(file).unwrap_or("application/octet-stream")
    };

    let start_ts = Timestamp::now();
    let mut input = CreateFileInput {
        parent,
        name: file_path.file_name().unwrap().to_string_lossy().to_string(),
        content_type: content_type.to_string(),
        size: Some(file_size),
        ..Default::default()
    };

    let mut fs = tokio::fs::File::open(&file_path)
        .await
        .map_err(format_error)?;
    // Indexed buckets can reject duplicates before accepting any chunks.
    // Bucket settings are only available on the full bucket canister.
    if file_size > MAX_FILE_SIZE_PER_CALL
        && cli
            .get_bucket_info()
            .await
            .is_ok_and(|info| info.enable_hash_index)
    {
        input.hash = Some(hash_file(&mut fs).await?.into());
        fs.rewind().await.map_err(format_error)?;
    }
    let on_progress = |progress: Progress| {
        let ts = ZonedDateTime::now()
            .strftime("%Y-%m-%d %H:%M:%S")
            .to_string();
        let percent = if file_size == 0 {
            100.0
        } else {
            progress.filled as f64 / file_size as f64 * 100.0
        };
        println!("{ts} uploaded: {percent:.2}%, {progress:?}");
    };
    let mut res = cli.upload(fs, input, on_progress).await?;

    let mut i = 0u8;
    while let Some(err) = res.error {
        // check before incrementing: `i > retry` can never hold for retry = u8::MAX,
        // and `i` would wrap around instead of ending the loop
        if i >= retry {
            return Err(format!("upload failed for file {}: {err}", res.id));
        }
        i += 1;

        println!(
            "upload error: {}.\ntry to resumable upload {} after 5s:",
            err, i
        );
        time::sleep(Duration::from_secs(5)).await;
        let fs = tokio::fs::File::open(&file_path)
            .await
            .map_err(format_error)?;
        res = cli
            .upload_chunks(
                fs,
                res.id,
                Some(file_size),
                None,
                &res.uploaded_chunks,
                on_progress,
            )
            .await;
    }

    println!(
        "upload success, file id: {}, size: {}, chunks: {}, retry: {}, time elapsed: {}",
        res.id,
        res.filled,
        res.uploaded_chunks.len(),
        i,
        Timestamp::now().duration_since(start_ts)
    );
    Ok(())
}

async fn hash_file(file: &mut tokio::fs::File) -> Result<[u8; 32], String> {
    let mut hasher = Sha3_256::new();
    let mut buf = vec![0; MAX_FILE_SIZE_PER_CALL as usize];
    loop {
        let len = file.read(&mut buf).await.map_err(format_error)?;
        if len == 0 {
            return Ok(hasher.finalize().into());
        }
        hasher.update(&buf[..len]);
    }
}

pub async fn download_file(cli: &Client, info: &FileInfo, path: &Path) -> Result<[u8; 32], String> {
    if info.size != info.filled {
        return Err("file not fully uploaded".to_string());
    }
    let mut file = tokio::fs::File::create_new(path)
        .await
        .map_err(format_error)?;
    let result = async {
        let mut hasher = Sha3_256::new();
        let mut filled = 0u64;
        let mut next = 0;
        let mut batches = futures::stream::iter((0..info.chunks).step_by(BATCH_CHUNKS as usize))
            .map(|index| cli.get_file_chunks(info.id, index, Some(BATCH_CHUNKS)))
            .buffered(4);
        while let Some(chunks) = batches.next().await {
            for chunk in chunks? {
                if chunk.0 != next {
                    return Err(format!("file chunk not found: {next}"));
                }
                next += 1;
                filled += chunk.1.len() as u64;
                hasher.update(&chunk.1);
                file.write_all(&chunk.1).await.map_err(format_error)?;
            }
            println!(
                "downloaded {next}/{} chunks ({filled}/{} bytes)",
                info.chunks, info.size
            );
        }
        if next != info.chunks {
            return Err(format!("file chunk not found: {next}"));
        }
        if filled != info.size {
            return Err(format!(
                "file size mismatch, expected {}, got {filled}",
                info.size
            ));
        }
        let hash: [u8; 32] = hasher.finalize().into();
        if let Some(expected) = info.hash.filter(|expected| **expected != hash) {
            return Err(format!(
                "file hash mismatch, expected {}, got {}",
                hex::encode(*expected),
                hex::encode(hash),
            ));
        }
        file.flush().await.map_err(format_error)?;
        Ok(hash)
    }
    .await;
    drop(file);

    if let Err(err) = &result {
        if let Err(cleanup) = tokio::fs::remove_file(path).await {
            return Err(format!(
                "{err}. Could not remove {} ({cleanup})",
                path.display()
            ));
        }
    }
    result
}
