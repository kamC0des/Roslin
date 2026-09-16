use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use walkdir::WalkDir;

#[derive(Default)]
pub struct AppState {
    pub staged_deletions: Mutex<HashMap<String, StagedBatch>>,
    pub next_stage_id: Mutex<u32>,
}

#[derive(Serialize, Clone)]
pub struct StagedBatch {
    pub paths: Vec<String>,
    pub reason: String,
}

pub fn list_files(directory: &str, recursive: bool) -> Value {
    const MAX_FILES: usize = 50; // Cap results

    if !std::path::Path::new(directory).exists() {
        return json!({ "error": format!("Directory does not exist: {}", directory) });
    }

    let mut results = vec![];
    let walker = if recursive {
        WalkDir::new(directory).into_iter()
    } else {
        WalkDir::new(directory).max_depth(1).into_iter()
    };

    for entry in walker.filter_map(|e| e.ok()) {
        if results.len() >= MAX_FILES {
            break; // Stop collecting once we hit the cap
        }
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                let modified = metadata.modified().ok();
                let age_years = modified.map(|m| {
                    m.elapsed().map(|d| d.as_secs_f64() / (365.25 * 24.0 * 3600.0)).unwrap_or(0.0)
                }).unwrap_or(0.0);

                results.push(json!({
                    "path": entry.path().to_string_lossy(),
                    "size_bytes": metadata.len(),
                    "age_years": (age_years * 10.0).round() / 10.0,
                }));
            }
        }
    }
    
    json!({ 
        "files": results,
        "truncated": results.len() >= MAX_FILES,
        "note": if results.len() >= MAX_FILES { "Showing first 50 files. Use find_files with keywords to narrow results." } else { "" }
    })
}

pub fn make_folder(path: &str) -> Value {
    match fs::create_dir_all(path) {
        Ok(_) => json!({ "created": path }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

pub fn move_files(paths: Vec<String>, destination_folder: &str) -> Value {
    let _ = fs::create_dir_all(destination_folder);
    let mut moved = vec![];
    let mut errors = vec![];

    for p in paths {
        let src = std::path::Path::new(&p);
        if !src.exists() {
            errors.push(format!("{}: not found", p));
            continue;
        }
        let file_name = src.file_name().unwrap();
        let dest = std::path::Path::new(destination_folder).join(file_name);
        match fs::rename(&src, &dest) {
            Ok(_) => moved.push(dest.to_string_lossy().to_string()),
            Err(e) => errors.push(format!("{}: {}", p, e)),
        }
    }
    json!({ "moved": moved, "errors": errors })
}

pub fn zip_files(paths: Vec<String>, output_zip_path: &str) -> Value {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let file = match fs::File::create(output_zip_path) {
        Ok(f) => f,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    let mut included = vec![];

    for p in &paths {
        let path = std::path::Path::new(p);
        if path.is_file() {
            if let Ok(contents) = fs::read(path) {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let _ = zip_writer.start_file(&name, options);
                let _ = zip_writer.write_all(&contents);
                included.push(p.clone());
            }
        }
    }
    let _ = zip_writer.finish();
    json!({ "zip_created": output_zip_path, "files_included": included })
}

pub fn create_file(path: &str, content: &str) -> Value {
    let p = std::path::Path::new(path);
    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match fs::write(p, content) {
        Ok(_) => json!({ "created": path }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

pub fn find_files(keywords: Vec<String>, directories: Vec<String>, recursive: bool) -> Value {
    if directories.is_empty() {
        return json!({ "error": "No directories provided to search." });
    }
    if keywords.is_empty() {
        return json!({ "error": "No search keywords provided." });
    }

    let keywords_lower: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    let mut matches = vec![];

    for dir in directories {
        if !std::path::Path::new(&dir).exists() {
            continue;
        }
        let walker = if recursive {
            WalkDir::new(&dir).into_iter()
        } else {
            WalkDir::new(&dir).max_depth(1).into_iter()
        };

        for entry in walker.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            
            // Match both files AND directories
            if keywords_lower.iter().all(|kw| name.contains(kw.as_str())) {
                if entry.file_type().is_file() {
                    if let Ok(metadata) = entry.metadata() {
                        matches.push(json!({
                            "path": entry.path().to_string_lossy(),
                            "type": "file",
                            "size_bytes": metadata.len(),
                        }));
                    }
                } else if entry.file_type().is_dir() {
                    // Count items in the directory
                    let item_count = std::fs::read_dir(entry.path())
                        .map(|entries| entries.count())
                        .unwrap_or(0);
                    
                    matches.push(json!({
                        "path": entry.path().to_string_lossy(),
                        "type": "directory",
                        "item_count": item_count,
                    }));
                }
            }
        }
    }

    json!({ "matches": matches, "count": matches.len() })
}

pub fn read_file(path: &str) -> Value {
    if path.is_empty() {
        return json!({ "error": "No file path provided." });
    }

    match std::fs::read_to_string(path) {
        Ok(content) => {
            // cap at 2000 chars to avoid bloating Claude's context
            let truncated = if content.len() > 2000 {
                format!("{}...[truncated]", &content[..2000])
            } else {
                content
            };
            json!({ "content": truncated, "path": path })
        }
        Err(e) => json!({ "error": format!("Could not read file: {}", e) })
    }
}


pub fn stage_deletion(state: &AppState, paths: Vec<String>, reason: String) -> Value {
    let mut resolved = vec![];
    let mut missing = vec![];
    for p in &paths {
        if std::path::Path::new(p).exists() {
            resolved.push(p.clone());
        } else {
            missing.push(p.clone());
        }
    }

    let mut id_counter = state.next_stage_id.lock().unwrap();
    let stage_id = format!("stage-{}", *id_counter);
    *id_counter += 1;

    state.staged_deletions.lock().unwrap().insert(
        stage_id.clone(),
        StagedBatch { paths: resolved.clone(), reason: reason.clone() },
    );

    json!({
        "stage_id": stage_id,
        "staged_count": resolved.len(),
        "staged_files": resolved,
        "missing": missing,
        "note": "Nothing deleted yet. Requires explicit human confirmation."
    })
}

pub fn list_staged_deletions(state: &AppState) -> Value {
    let staged = state.staged_deletions.lock().unwrap();
    //json!({ "staged": *staged as *const _ }); // placeholder replaced below
    let map: HashMap<String, Value> = staged.iter().map(|(k, v)| {
        (k.clone(), json!({ "paths": v.paths, "reason": v.reason }))
    }).collect();
    json!({ "staged": map })
}

pub fn execute_staged_deletion(state: &AppState, stage_id: &str, human_confirmed: bool) -> Value {
    if !human_confirmed {
        return json!({ "error": "Execution blocked: no explicit human confirmation for this turn." });
    }

    let mut staged = state.staged_deletions.lock().unwrap();
    let Some(batch) = staged.remove(stage_id) else {
        return json!({ "error": format!("No staged deletion with id {}", stage_id) });
    };

    let mut deleted = vec![];
    let mut errors = vec![];
    for p in batch.paths {
        match fs::remove_file(&p) {
            Ok(_) => deleted.push(p),
            Err(e) => errors.push(format!("{}: {}", p, e)),
        }
    }
    json!({ "deleted": deleted, "errors": errors })
}