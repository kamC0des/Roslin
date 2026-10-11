// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod tools;
use tools::AppState;
use tauri::State;
use serde_json::{json, Value};
use std::path::PathBuf;
use walkdir::WalkDir;
use keyring::Entry;


const MAX_ACTIVE_MESSAGES: usize = 20; // keep last 20 messages in context

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_files",
            "description": "List files in a directory with size and age in years. Use to find files matching criteria before acting.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "directory": { "type": "string" },
                    "recursive": { "type": "boolean" }
                },
                "required": ["directory"]
            }
        },
        {
            "name": "move_files",
            "description": "Move files into a destination folder, creating it if needed.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "destination_folder": { "type": "string" }
                },
                "required": ["paths", "destination_folder"]
            }
        },
        {
            "name": "zip_files",
            "description": "Compress files into a single .zip archive.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "output_zip_path": { "type": "string" }
                },
                "required": ["paths", "output_zip_path"]
            }
        },
        {
            "name": "stage_deletion",
            "description": "Queue files for deletion. Does NOT delete. Must be followed by showing the user what's staged and getting explicit confirmation before calling execute_staged_deletion.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "reason": { "type": "string" }
                },
                "required": ["paths", "reason"]
            }
        },
        {
            "name": "execute_staged_deletion",
            "description": "Permanently delete a staged batch by stage_id. Only call this if the user's latest message is an explicit confirmation (e.g. 'yes', 'confirm').",
            "input_schema": {
                "type": "object",
                "properties": { "stage_id": { "type": "string" } },
                "required": ["stage_id"]
            }
        },
        {
    "name": "create_file",
    "description": "Create a new text-based file (.txt, .py, .md, .json, .csv, .html, etc.) with given content. Does NOT support .docx, .pdf, or .doc — those require a different tool not yet available.",
    "input_schema": {
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Full absolute Windows path including filename and extension." },
            "content": { "type": "string" }
        },
        "required": ["path", "content"]
    }
    },
    {
    "name": "find_files",
    "description": "Search for files and folders by filename (case-insensitive, partial match) across one or more directories. Returns all matching paths if there are multiple.",
    "input_schema": {
        "type": "object",
        "properties": {
            "keywords": { "type": "array", "items": { "type": "string" }, "description": "Individual search terms, e.g. ['spotify', 'resume']." },
            "directories": { "type": "array", "items": { "type": "string" }, "description": "Absolute Windows paths to search in." },
            "recursive": { "type": "boolean" }
        },
        "required": ["keywords", "directories"]
        }
    },
    {
    "name": "read_file",
    "description": "Read the full text content of a specific file (up to 2000 chars). Use this only when you need to inspect a file's contents to answer the user's question, e.g. checking which invoice is unpaid. Do NOT call this in bulk; call it on specific files the user asks about.",
    "input_schema": {
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Absolute Windows path to the file." }
        },
        "required": ["path"]
    }
    },
    {
    "name": "rename_file",
    "description": "Rename a file or folder. Provide the full path and the new name (just the name, not a full path).",
    "input_schema": {
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "Full absolute path to the file or folder to rename." },
            "new_name": { "type": "string", "description": "The new name (e.g., 'resume_updated.txt', not a full path)." }
        },
        "required": ["path", "new_name"]
    }
    }
    
    ])
}




fn get_conversation_file() -> Result<PathBuf, String> {
    let data_dir = dirs::data_local_dir()
        .ok_or("Could not find local data directory")?;
    let roslin_dir = data_dir.join("Roslin");
    std::fs::create_dir_all(&roslin_dir).map_err(|e| e.to_string())?;
    Ok(roslin_dir.join("conversation.json"))
}



#[tauri::command]
async fn set_api_key(key: String) -> Result<(), String> {
   if key.trim().is_empty() {
        return Err("API key cannot be empty".to_string());
    }
    
    // Validate the key by making a test API call
    let client = reqwest::Client::new();
    let res = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-6",
            "max_tokens": 10,
            "messages": [{ "role": "user", "content": "test" }],
        }))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    // Check if the API call succeeded
    if !res.status().is_success() {
        let status = res.status();
        if status == 401 {
            return Err("Invalid API key. Check that your key is correct.".to_string());
        }
        return Err(format!("API error: {}", status));
    }

    // Only save if validation passed
    Entry::new("Roslin", "anthropic_api_key")
        .map_err(|e| e.to_string())?
        .set_password(&key)
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
fn get_api_key() -> Result<String, String> {
    Entry::new("Roslin", "anthropic_api_key")
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())
}



#[tauri::command]
fn save_conversation(conversation: serde_json::Value) -> Result<(), String> {
    let file_path = get_conversation_file()?;
    
    // Keep full history for display/replay, but trim active context
    let messages = conversation.as_array().ok_or("Invalid conversation format")?;
    
    // Only send last MAX_ACTIVE_MESSAGES to the API, but save everything
    let trimmed: Vec<_> = messages.iter()
        .skip(messages.len().saturating_sub(MAX_ACTIVE_MESSAGES))
        .cloned()
        .collect();
    
    let to_save = json!({
        "full_history": messages,  // keep everything for reference
        "active_context": trimmed   // this is what gets sent to Claude next time
    });
    
    let json_string = serde_json::to_string_pretty(&to_save).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, json_string).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_conversation() -> Result<serde_json::Value, String> {
    let file_path = get_conversation_file()?;
    if !file_path.exists() {
        return Ok(serde_json::json!([]));
    }
    let content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    
    // Return active_context if it exists (new format), otherwise full_history (old format for backwards compat)
    if let Some(active) = parsed.get("active_context") {
        Ok(active.clone())
    } else if let Some(full) = parsed.get("full_history") {
        Ok(full.clone())
    } else {
        Ok(parsed) // fallback for old single-array format
    }
}

#[tauri::command]
fn clear_conversation() -> Result<(), String> {
    let file_path = get_conversation_file()?;
    std::fs::remove_file(&file_path).map_err(|e| e.to_string())?;
    Ok(())
}




#[tauri::command]
async fn claude_call(
    system_prompt: String,
    messages: String,
    human_confirmed: bool,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    //let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|e| e.to_string())?;
    let api_key = get_api_key()?; // Retrieve from Credential Manager instead of env var
    let client = reqwest::Client::new();
    let mut messages: Value = serde_json::from_str(&messages).map_err(|e| e.to_string())?;

    loop {
        let res = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&json!({
                "model": "claude-sonnet-4-6",
                "max_tokens": 1024,
                "system": system_prompt,
                "tools": tool_definitions(),
                "messages": messages,
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let response_json: Value = res.json().await.map_err(|e| e.to_string())?;
        let content = response_json["content"].clone();

        // DEBUG: print the actual response structure
        eprintln!("Response JSON: {}", serde_json::to_string_pretty(&response_json).unwrap_or_default());
        eprintln!("Content: {}", serde_json::to_string_pretty(&content).unwrap_or_default());
        // append assistant turn to running conversation
        messages.as_array_mut().unwrap().push(json!({
            "role": "assistant",
            "content": content.clone()
        }));

        let tool_calls: Vec<&Value> = content.as_array().unwrap().iter()
            .filter(|b| b["type"] == "tool_use")
            .collect();

        if tool_calls.is_empty() {
            let text = content.as_array().unwrap().iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
                .unwrap_or("(no text response)")
                .to_string();

            return Ok(json!({ "text": text, "messages": messages }));
        }

        let mut tool_results = vec![];
        for call in tool_calls {
            let name = call["name"].as_str().unwrap_or("");
            let input = &call["input"];
            let id = call["id"].as_str().unwrap_or("");

            let result = match name {
                "create_file" => tools::create_file(
                    input["path"].as_str().unwrap_or(""),
                    input["content"].as_str().unwrap_or(""),
                ),
                "rename_file" => tools::rename_file(
                    input["path"].as_str().unwrap_or(""),
                    input["new_name"].as_str().unwrap_or(""),
                ),
                "read_file" => tools::read_file(input["path"].as_str().unwrap_or("")),
                "find_files" => tools::find_files(
                   input["keywords"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_else(Vec::new),
                    input["directories"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_else(Vec::new),
                    input["recursive"].as_bool().unwrap_or(true),
                ),
                "list_files" => tools::list_files(
                    input["directory"].as_str().unwrap_or("."),
                    input["recursive"].as_bool().unwrap_or(false),
                ),
                "move_files" => {
                    let paths = if let Some(arr) = input["paths"].as_array() {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    } else if let Some(str_val) = input["paths"].as_str() {
                        // Try parsing as-is first
                        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(str_val) {
                            parsed
                        } else {
                            // If parsing fails, try fixing unescaped backslashes
                            let fixed = str_val.replace("\\", "\\\\");
                            serde_json::from_str::<Vec<String>>(&fixed).unwrap_or_default()
                        }
                    } else {
                        vec![]
                    };
                    
                    tools::move_files(
                        paths,
                        input["destination_folder"].as_str().unwrap_or(""),
                    )
                },
                "zip_files" => {
                    let paths = if let Some(arr) = input["paths"].as_array() {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    } else if let Some(str_val) = input["paths"].as_str() {
                        // Handle stringified JSON array
                        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(str_val) {
                            parsed
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    };
                    
                    tools::zip_files(
                        paths,
                        input["output_zip_path"].as_str().unwrap_or(""),
                    )
                },
                "stage_deletion" => {
                    let paths = if let Some(arr) = input["paths"].as_array() {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    } else if let Some(str_val) = input["paths"].as_str() {
                        // Handle stringified JSON array
                        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(str_val) {
                            parsed
                        } else {
                            // Fix unescaped backslashes
                            let fixed = str_val.replace("\\", "\\\\");
                            serde_json::from_str::<Vec<String>>(&fixed).unwrap_or_default()
                        }
                    } else {
                        vec![]
                    };
                    
                    tools::stage_deletion(
                        &state,
                        paths,
                        input["reason"].as_str().unwrap_or("").to_string(),
                    )
                },
                "execute_staged_deletion" => {

                    let paths = if let Some(arr) = input["paths"].as_array() {
                        arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                    } else if let Some(str_val) = input["paths"].as_str() {
                        // Handle stringified JSON array
                        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(str_val) {
                            parsed
                        } else {
                            // Fix unescaped backslashes
                            let fixed = str_val.replace("\\", "\\\\");
                            serde_json::from_str::<Vec<String>>(&fixed).unwrap_or_default()
                        }
                    } else {
                        vec![]
                    };
                    
                    
                    tools::execute_staged_deletion(
                        &state,
                        input["stage_id"].as_str().unwrap_or(""),
                        human_confirmed,
                    )
            
                },
                _ => json!({ "error": format!("Unknown tool: {}", name) }),
            };

            tool_results.push(json!({
                "type": "tool_result",
                "tool_use_id": id,
                "content": result.to_string()
            }));
        }

        messages.as_array_mut().unwrap().push(json!({
            "role": "user",
            "content": tool_results
        }));
    }
}
#[tauri::command]
fn debug_find(directory: &str) -> Value {
    let mut dirs = vec![];
    let mut files = vec![];
    
    for entry in WalkDir::new(directory).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type().is_dir() {
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    
    json!({ 
        "directories": dirs,
        "directory_count": dirs.len(),
        "file_count": files.len(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![claude_call, load_conversation, save_conversation, clear_conversation, debug_find, set_api_key, get_api_key])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
