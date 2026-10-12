//! Real-file oracle: every model file StarUML ships must load and save back
//! unchanged. The files are read from a local StarUML install and never
//! committed. Point `STARUML_ASAR` at `resources/app.asar`; the test is
//! skipped (with a note) when no install is found.

// Test code: failing loudly is the point.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use mdj::Document;
use serde_json::Value;

const DEFAULT_PATHS: &[&str] = &[
    "/mnt/c/Program Files/StarUML/resources/app.asar",
    "C:\\Program Files\\StarUML\\resources\\app.asar",
    "/Applications/StarUML.app/Contents/Resources/app.asar",
    "/opt/StarUML/resources/app.asar",
];

fn find_asar() -> Option<PathBuf> {
    std::env::var_os("STARUML_ASAR")
        .map(PathBuf::from)
        .into_iter()
        .chain(DEFAULT_PATHS.iter().map(PathBuf::from))
        .find(|p| p.is_file())
}

/// Minimal asar reader: an 16-byte pickle prefix, a JSON table of contents,
/// then file bytes at `8 + header_size + offset`.
fn asar_model_files(path: &PathBuf) -> Vec<(String, Vec<u8>)> {
    let bytes = std::fs::read(path).expect("read asar");
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
    let header_size = u32_at(4);
    let json_len = u32_at(12);
    let header: Value = serde_json::from_slice(&bytes[16..16 + json_len]).expect("asar header");
    let base = 8 + header_size;

    let mut out = Vec::new();
    let mut stack = vec![(String::new(), &header)];
    while let Some((prefix, node)) = stack.pop() {
        let Some(files) = node.get("files").and_then(Value::as_object) else {
            continue;
        };
        for (name, entry) in files {
            let full = format!("{prefix}/{name}");
            if name == "node_modules" {
                continue;
            }
            if entry.get("files").is_some() {
                stack.push((full, entry));
            } else if name.ends_with(".mdj") || name.ends_with(".mfj") {
                let offset: usize = entry["offset"].as_str().unwrap().parse().unwrap();
                let size = entry["size"].as_u64().unwrap() as usize;
                out.push((full, bytes[base + offset..base + offset + size].to_vec()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn shipped_model_files_round_trip_losslessly() {
    let Some(asar) = find_asar() else {
        eprintln!("skipped: no StarUML install found (set STARUML_ASAR)");
        return;
    };
    let files = asar_model_files(&asar);
    assert!(
        files.len() >= 10,
        "expected StarUML's templates, found {}",
        files.len()
    );

    for (name, bytes) in &files {
        let text = std::str::from_utf8(bytes).unwrap();
        let original: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
        let doc = Document::from_json_str(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let saved = doc.to_json_string().unwrap();
        let reparsed: Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(reparsed, original, "{name} changed on round trip");
        // Same key order too, so diffs against StarUML's own output stay small.
        assert_eq!(
            serde_json::to_string(&reparsed).unwrap(),
            serde_json::to_string(&original).unwrap(),
            "{name}: key order changed"
        );
        eprintln!("ok  {name}  ({} elements)", doc.len());
    }
}
