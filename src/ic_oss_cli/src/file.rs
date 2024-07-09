use chrono::prelude::*;
use ic_oss_types::{file::*, format_error};
use tokio::{time, time::Duration};

pub async fn upload_file(cli: &ic_oss::file::Client, file: &str, retry: u8) -> Result<(), String> {
    let start_ts: DateTime<Local> = Local::now();
    let file_path = std::path::Path::new(file);
    let metadata = std::fs::metadata(file_path).map_err(format_error)?;
    if !metadata.is_file() {
        return Err(format!("not a file: {:?}", file));
    }

    let file_size = metadata.len();
    let content_type = infer::get_from_path(file_path)
        .map_err(format_error)?
        .map(|f| f.mime_type());

    let content_type = if let Some(content_type) = content_type {
        content_type
    } else {
        mime_db::lookup(file).unwrap_or("application/octet-stream")
    };

    let input = CreateFileInput {
        name: file_path.file_name().unwrap().to_string_lossy().to_string(),
        content_type: content_type.to_string(),
        size: Some(file_size),
        ..Default::default()
    };
    let fs = tokio::fs::File::open(&file_path)
        .await
        .map_err(format_error)?;
    let mut res = cli
        .upload(fs, input, move |filled| {
            let ts: DateTime<Local> = Local::now();
            let ts = ts.format("%Y-%m-%d %H:%M:%S").to_string();
            println!(
                "{} uploaded: {:.2}%",
                ts,
                (filled as f32 / file_size as f32) * 100.0
            );
        })
        .await
        .map_err(format_error)?;

    let mut i = 0u8;
    while let Some(err) = res.error {
        i += 1;
        if i > retry {
            return Err(format!("upload failed: {}", err));
        }

        println!(
            "upload error: {}.\ntry to resumable upload {} after 5s:",
            err, i
        );
        time::sleep(Duration::from_secs(5)).await;
        let fs = tokio::fs::File::open(&file_path)
            .await
            .map_err(format_error)?;
        res = cli
            .upload_chunks(fs, res.id, &res.uploaded_chunks, move |filled| {
                let ts: DateTime<Local> = Local::now();
                let ts = ts.format("%Y-%m-%d %H:%M:%S").to_string();
                println!(
                    "{} uploaded: {:.2}%",
                    ts,
                    (filled as f32 / file_size as f32) * 100.0
                );
            })
            .await;
    }

    println!(
        "upload success, file id: {}, size: {}, chunks: {}, retry: {}, time elapsed: {}",
        res.id,
        res.uploaded,
        res.uploaded_chunks.len(),
        i,
        Local::now().signed_duration_since(start_ts)
    );
    Ok(())
}
