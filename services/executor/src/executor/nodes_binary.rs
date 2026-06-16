// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! Binary-processing utility nodes. Binary payloads cross the JSON boundary as
//! base64. zip / image / pdf use pure-Rust crates; OCR shells out to the
//! `tesseract` CLI at runtime so the workspace needs no system library to
//! build (the binary is only required when an OCR node actually runs).

use super::*;
use crate::runtime::{ExecutionContext, NodeExecutionResult};
use base64::Engine as _;
use std::io::{Cursor, Read, Write};
use workflow_core::Node;

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn decode_b64(node: &str, field: &str, s: &str) -> Result<Vec<u8>, NodeExecutionResult> {
    b64().decode(s.trim()).map_err(|e| {
        NodeExecutionResult::failed(format!("{node} '{field}' is not valid base64: {e}"))
    })
}

// ── Zip (create / extract) ────────────────────────────────────────────────────
pub(super) async fn execute_zip(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("zip")
        .to_string();

    match operation.as_str() {
        "zip" => {
            let files = match cfg.get("files").map(json_array_or_parse) {
                Some(serde_json::Value::Array(a)) => a,
                _ => {
                    return NodeExecutionResult::failed(
                        "Zip 'files' must be an array of {name, content[, base64]}",
                    )
                }
            };
            let mut buf = Cursor::new(Vec::new());
            {
                let mut writer = zip::ZipWriter::new(&mut buf);
                let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                for (i, f) in files.iter().enumerate() {
                    let name = match f.get("name").and_then(|v| v.as_str()) {
                        Some(n) if !n.is_empty() => n.to_string(),
                        _ => {
                            return NodeExecutionResult::failed(format!(
                                "Zip files[{i}] needs 'name'"
                            ))
                        }
                    };
                    let raw_content = f.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    // content is UTF-8 text unless base64:true is set.
                    let bytes = if f.get("base64").and_then(|v| v.as_bool()) == Some(true) {
                        match decode_b64("Zip", "content", raw_content) {
                            Ok(b) => b,
                            Err(e) => return e,
                        }
                    } else {
                        raw_content.as_bytes().to_vec()
                    };
                    if let Err(e) = writer.start_file(name, opts) {
                        return NodeExecutionResult::failed(format!("Zip start_file error: {e}"));
                    }
                    if let Err(e) = writer.write_all(&bytes) {
                        return NodeExecutionResult::failed(format!("Zip write error: {e}"));
                    }
                }
                if let Err(e) = writer.finish() {
                    return NodeExecutionResult::failed(format!("Zip finalize error: {e}"));
                }
            }
            let data = buf.into_inner();
            NodeExecutionResult::succeeded(
                serde_json::json!({
                    "zip_base64": b64().encode(&data),
                    "file_count": files.len(),
                    "size": data.len(),
                })
                .to_string(),
            )
        }
        "unzip" => {
            let zip_b64 = match cfg.get("zip_base64").and_then(|v| v.as_str()) {
                Some(z) if !z.is_empty() => z.to_string(),
                _ => return NodeExecutionResult::failed("Unzip requires 'zip_base64'"),
            };
            let bytes = match decode_b64("Unzip", "zip_base64", &zip_b64) {
                Ok(b) => b,
                Err(e) => return e,
            };
            let mut archive = match zip::ZipArchive::new(Cursor::new(bytes)) {
                Ok(a) => a,
                Err(e) => return NodeExecutionResult::failed(format!("Unzip open error: {e}")),
            };
            let mut entries = Vec::new();
            for i in 0..archive.len() {
                let mut file = match archive.by_index(i) {
                    Ok(f) => f,
                    Err(e) => {
                        return NodeExecutionResult::failed(format!("Unzip entry error: {e}"))
                    }
                };
                if file.is_dir() {
                    continue;
                }
                let name = file.name().to_string();
                let mut content = Vec::new();
                if let Err(e) = file.read_to_end(&mut content) {
                    return NodeExecutionResult::failed(format!("Unzip read error: {e}"));
                }
                entries.push(serde_json::json!({
                    "name": name,
                    "content_base64": b64().encode(&content),
                    "size": content.len(),
                }));
            }
            NodeExecutionResult::succeeded(
                serde_json::json!({ "files": entries, "file_count": entries.len() }).to_string(),
            )
        }
        other => NodeExecutionResult::failed(format!("Zip unknown operation '{other}'")),
    }
}

// ── Image (resize / convert / metadata) ───────────────────────────────────────
pub(super) async fn execute_image(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let operation = cfg
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("metadata")
        .to_string();
    let image_b64 = match cfg.get("image_base64").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("Image requires 'image_base64'"),
    };
    let bytes = match decode_b64("Image", "image_base64", &image_b64) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i,
        Err(e) => return NodeExecutionResult::failed(format!("Image decode error: {e}")),
    };

    let encode =
        |img: &image::DynamicImage, fmt: &str| -> Result<(Vec<u8>, &'static str), String> {
            let format = match fmt.to_lowercase().as_str() {
                "png" => image::ImageFormat::Png,
                "jpeg" | "jpg" => image::ImageFormat::Jpeg,
                "gif" => image::ImageFormat::Gif,
                "bmp" => image::ImageFormat::Bmp,
                "webp" => image::ImageFormat::WebP,
                other => return Err(format!("unsupported format '{other}'")),
            };
            let mut out = Cursor::new(Vec::new());
            img.write_to(&mut out, format)
                .map_err(|e| format!("encode error: {e}"))?;
            let label = match format {
                image::ImageFormat::Png => "png",
                image::ImageFormat::Jpeg => "jpeg",
                image::ImageFormat::Gif => "gif",
                image::ImageFormat::Bmp => "bmp",
                _ => "webp",
            };
            Ok((out.into_inner(), label))
        };

    match operation.as_str() {
        "metadata" => NodeExecutionResult::succeeded(
            serde_json::json!({
                "width": img.width(),
                "height": img.height(),
                "color": format!("{:?}", img.color()),
            })
            .to_string(),
        ),
        "resize" => {
            let width = cfg.get("width").and_then(|v| v.as_u64()).map(|v| v as u32);
            let height = cfg.get("height").and_then(|v| v.as_u64()).map(|v| v as u32);
            let (w, h) = match (width, height) {
                (Some(w), Some(h)) => (w, h),
                (Some(w), None) => {
                    // preserve aspect ratio from width
                    let h = (img.height() as f64 * (w as f64 / img.width() as f64)).round() as u32;
                    (w, h.max(1))
                }
                (None, Some(h)) => {
                    let w = (img.width() as f64 * (h as f64 / img.height() as f64)).round() as u32;
                    (w.max(1), h)
                }
                (None, None) => {
                    return NodeExecutionResult::failed("Image resize requires 'width' or 'height'")
                }
            };
            let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
            let fmt = cfg.get("format").and_then(|v| v.as_str()).unwrap_or("png");
            match encode(&resized, fmt) {
                Ok((data, label)) => NodeExecutionResult::succeeded(
                    serde_json::json!({
                        "image_base64": b64().encode(&data),
                        "format": label,
                        "width": w,
                        "height": h,
                    })
                    .to_string(),
                ),
                Err(e) => NodeExecutionResult::failed(format!("Image resize {e}")),
            }
        }
        "convert" => {
            let fmt = match cfg.get("format").and_then(|v| v.as_str()) {
                Some(f) if !f.is_empty() => f,
                _ => return NodeExecutionResult::failed("Image convert requires 'format'"),
            };
            match encode(&img, fmt) {
                Ok((data, label)) => NodeExecutionResult::succeeded(
                    serde_json::json!({
                        "image_base64": b64().encode(&data),
                        "format": label,
                        "width": img.width(),
                        "height": img.height(),
                    })
                    .to_string(),
                ),
                Err(e) => NodeExecutionResult::failed(format!("Image convert {e}")),
            }
        }
        other => NodeExecutionResult::failed(format!("Image unknown operation '{other}'")),
    }
}

// ── PDF text extraction ───────────────────────────────────────────────────────
pub(super) async fn execute_pdf_extract(
    node: &Node,
    context: &ExecutionContext,
) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let pdf_b64 = match cfg.get("pdf_base64").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("PDF extract requires 'pdf_base64'"),
    };
    let bytes = match decode_b64("PDF", "pdf_base64", &pdf_b64) {
        Ok(b) => b,
        Err(e) => return e,
    };
    // pdf-extract can panic on some malformed inputs; isolate it.
    let result = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem(&bytes));
    match result {
        Ok(Ok(text)) => {
            let chars = text.chars().count();
            NodeExecutionResult::succeeded(
                serde_json::json!({ "text": text, "char_count": chars }).to_string(),
            )
        }
        Ok(Err(e)) => NodeExecutionResult::failed(format!("PDF extract error: {e}")),
        Err(_) => NodeExecutionResult::failed("PDF extract failed (malformed or unsupported PDF)"),
    }
}

// ── OCR (tesseract CLI) ───────────────────────────────────────────────────────
pub(super) async fn execute_ocr(node: &Node, context: &ExecutionContext) -> NodeExecutionResult {
    let raw = node.config.clone().unwrap_or_default();
    let cfg = resolve_config_strings(&raw, context);
    let image_b64 = match cfg.get("image_base64").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return NodeExecutionResult::failed("OCR requires 'image_base64'"),
    };
    let lang = cfg
        .get("lang")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("eng")
        .to_string();
    let bytes = match decode_b64("OCR", "image_base64", &image_b64) {
        Ok(b) => b,
        Err(e) => return e,
    };

    // Persist to a temp file because the tesseract CLI reads from a path.
    let mut path = std::env::temp_dir();
    let unique = format!(
        "trigix-ocr-{}-{}.img",
        context.execution_id,
        node.id.replace(['/', '\\'], "_")
    );
    path.push(unique);
    if let Err(e) = std::fs::write(&path, &bytes) {
        return NodeExecutionResult::failed(format!("OCR temp write error: {e}"));
    }

    let output = std::process::Command::new("tesseract")
        .arg(&path)
        .arg("stdout")
        .arg("-l")
        .arg(&lang)
        .output();
    let _ = std::fs::remove_file(&path);

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            NodeExecutionResult::succeeded(
                serde_json::json!({ "text": text, "lang": lang }).to_string(),
            )
        }
        Ok(out) => NodeExecutionResult::failed(format!(
            "OCR tesseract failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )),
        Err(e) => NodeExecutionResult::failed(format!(
            "OCR could not run 'tesseract' ({e}); install the tesseract CLI to use this node"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workflow_core::{Node, NodeType};

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            execution_id: "e1".into(),
            workflow_version_id: "v1".into(),
            input_json: "{}".into(),
            node_outputs: Default::default(),
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn zip_roundtrips_a_text_file() {
        let zip_node = Node {
            id: "z1".into(),
            node_type: NodeType::Zip,
            config: Some(serde_json::json!({
                "operation":"zip",
                "files":[{"name":"hello.txt","content":"hello world"}]
            })),
        };
        let zipped = execute_zip(&zip_node, &ctx()).await;
        let zip_b64 =
            serde_json::from_str::<serde_json::Value>(zipped.output_json.as_deref().unwrap())
                .unwrap()["zip_base64"]
                .as_str()
                .unwrap()
                .to_string();

        let unzip_node = Node {
            id: "z2".into(),
            node_type: NodeType::Zip,
            config: Some(serde_json::json!({"operation":"unzip","zip_base64":zip_b64})),
        };
        let out: serde_json::Value = serde_json::from_str(
            execute_zip(&unzip_node, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["file_count"], 1);
        assert_eq!(out["files"][0]["name"], "hello.txt");
        let content = base64::engine::general_purpose::STANDARD
            .decode(out["files"][0]["content_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(String::from_utf8(content).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn image_metadata_and_convert_roundtrip() {
        // Build a tiny 2x3 RGB image and PNG-encode it as input.
        let mut img = image::RgbImage::new(2, 3);
        img.put_pixel(0, 0, image::Rgb([255, 0, 0]));
        let dynimg = image::DynamicImage::ImageRgb8(img);
        let mut png = Cursor::new(Vec::new());
        dynimg.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let png_b64 = base64::engine::general_purpose::STANDARD.encode(png.into_inner());

        let meta = Node {
            id: "i1".into(),
            node_type: NodeType::Image,
            config: Some(
                serde_json::json!({"operation":"metadata","image_base64":png_b64.clone()}),
            ),
        };
        let out: serde_json::Value = serde_json::from_str(
            execute_image(&meta, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["width"], 2);
        assert_eq!(out["height"], 3);

        let conv = Node {
            id: "i2".into(),
            node_type: NodeType::Image,
            config: Some(serde_json::json!({
                "operation":"convert","format":"jpeg","image_base64":png_b64
            })),
        };
        let out2: serde_json::Value = serde_json::from_str(
            execute_image(&conv, &ctx())
                .await
                .output_json
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out2["format"], "jpeg");
        assert!(out2["image_base64"].as_str().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn image_requires_input() {
        let n = Node {
            id: "i3".into(),
            node_type: NodeType::Image,
            config: Some(serde_json::json!({"operation":"metadata"})),
        };
        let r = execute_image(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("image_base64"));
    }

    #[tokio::test]
    async fn pdf_requires_input() {
        let n = Node {
            id: "p1".into(),
            node_type: NodeType::PdfExtract,
            config: Some(serde_json::json!({})),
        };
        let r = execute_pdf_extract(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("pdf_base64"));
    }

    #[tokio::test]
    async fn ocr_requires_input() {
        let n = Node {
            id: "o1".into(),
            node_type: NodeType::Ocr,
            config: Some(serde_json::json!({"lang":"eng"})),
        };
        let r = execute_ocr(&n, &ctx()).await;
        assert!(r.error.as_deref().unwrap_or("").contains("image_base64"));
    }
}
