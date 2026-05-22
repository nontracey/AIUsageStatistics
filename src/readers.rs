use crate::models::*;
use chrono::{NaiveDate, Timelike};
use std::path::PathBuf;
use std::sync::mpsc;

pub trait UsageReader: Send {
    fn name(&self) -> &str;
    fn is_installed(&self) -> bool;
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord>;
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"))
}

fn local_share_dir() -> PathBuf {
    home_dir().join(".local/share")
}

fn is_db_valid(path: &std::path::Path) -> bool {
    path.exists() && path.metadata().map(|m| m.len() > 100).unwrap_or(false)
}

// ─── opencode ─────────────────────────────────────────────────

pub struct OpencodeReader;

impl UsageReader for OpencodeReader {
    fn name(&self) -> &str { "opencode" }
    fn is_installed(&self) -> bool {
        which_installed("opencode") || local_share_dir().join("opencode/opencode.db").exists()
    }
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord> {
        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.1, message: "Opening database...".into(), done: false, error: None,
        });

        let db_path = local_share_dir().join("opencode/opencode.db");
        if !is_db_valid(&db_path) {
            let _ = progress.send(ProgressUpdate {
                cli_name: self.name().to_string(), percent: 1.0, message: "Database not found".into(), done: true, error: Some("Database not found".into()),
            });
            return vec![];
        }

        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: 1.0, message: format!("Error: {e}"), done: true, error: Some(e.to_string()),
                });
                return vec![];
            }
        };

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.3, message: "Querying sessions...".into(), done: false, error: None,
        });

        let start_str = range.start.format("%Y-%m-%d").to_string();
        let end_str = range.end.format("%Y-%m-%d").to_string();

        let mut stmt = match conn.prepare(
            "SELECT tokens_input, tokens_output, tokens_cache_read, tokens_cache_write,
                    tokens_reasoning, model, time_created, cost, agent
             FROM session
             WHERE date(time_created / 1000, 'unixepoch') >= ?1
               AND date(time_created / 1000, 'unixepoch') <= ?2
             ORDER BY time_created"
        ) {
            Ok(s) => s,
            Err(e) => {
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: 1.0, message: format!("Query error: {e}"), done: true, error: Some(e.to_string()),
                });
                return vec![];
            }
        };

        let rows = stmt.query_map(rusqlite::params![start_str, end_str], |row| {
            let tokens_input: i64 = row.get(0)?;
            let tokens_output: i64 = row.get(1)?;
            let cache_read: i64 = row.get(2)?;
            let cache_write: i64 = row.get(3)?;
            let reasoning: i64 = row.get(4)?;
            let model_json: String = row.get(5)?;
            let ts_ms: i64 = row.get(6)?;
            let cost: f64 = row.get(7)?;
            let _agent: Option<String> = row.get(8)?;

            let model_name = serde_json::from_str::<serde_json::Value>(&model_json)
                .ok()
                .and_then(|v| v.get("id").and_then(|id| id.as_str().map(|s| s.to_string())))
                .unwrap_or_else(|| model_json.clone());

            let dt = chrono::DateTime::from_timestamp(ts_ms / 1000, 0)
                .map(|d| d.naive_utc())
                .unwrap_or_default();

            Ok(UsageRecord {
                cli_name: "opencode".to_string(),
                source_type: SourceType::Cli,
                model_name,
                prompt_tokens: tokens_input.max(0) as u64,
                completion_tokens: tokens_output.max(0) as u64,
                total_tokens: (tokens_input + tokens_output).max(0) as u64,
                cache_read_tokens: (cache_read.max(0) as u64).min(tokens_input.max(0) as u64),
                cache_write_tokens: cache_write.max(0) as u64,
                reasoning_tokens: reasoning.max(0) as u64,
                request_count: 1,
                cost_cents: (cost * 100.0) as u64,
                date: dt.date(),
                hour: dt.hour() as u8,
                timestamp: dt,
            })
        });

        let mut records = Vec::new();
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                records.push(row);
            }
        }

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 1.0,
            message: format!("Done: {} records, {} tokens", records.len(), format_tokens_full(records.iter().map(|r| r.total_tokens).sum())),
            done: true, error: None,
        });
        records
    }
}

// ─── hermes ───────────────────────────────────────────────────

pub struct HermesReader;

impl UsageReader for HermesReader {
    fn name(&self) -> &str { "hermes" }
    fn is_installed(&self) -> bool {
        which_installed("hermes") || home_dir().join(".hermes/state.db").exists()
    }
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord> {
        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.1, message: "Opening database...".into(), done: false, error: None,
        });

        let db_path = home_dir().join(".hermes/state.db");
        if !db_path.exists() {
            let _ = progress.send(ProgressUpdate {
                cli_name: self.name().to_string(), percent: 1.0, message: "Database not found".into(), done: true, error: Some("Database not found".into()),
            });
            return vec![];
        }

        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: 1.0, message: format!("Error: {e}"), done: true, error: Some(e.to_string()),
                });
                return vec![];
            }
        };

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.3, message: "Querying sessions...".into(), done: false, error: None,
        });

        let start_ts = range.start.and_hms_opt(0, 0, 0).map(|d| d.and_utc().timestamp() as f64).unwrap_or(0.0);
        let end_ts = range.end.and_hms_opt(23, 59, 59).map(|d| d.and_utc().timestamp() as f64).unwrap_or(0.0);

        let mut stmt = match conn.prepare(
            "SELECT model, input_tokens, output_tokens, cache_read_tokens,
                    cache_write_tokens, reasoning_tokens, started_at,
                    estimated_cost_usd, actual_cost_usd, source
             FROM sessions
             WHERE started_at >= ?1 AND started_at <= ?2
             ORDER BY started_at"
        ) {
            Ok(s) => s,
            Err(e) => {
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: 1.0, message: format!("Query error: {e}"), done: true, error: Some(e.to_string()),
                });
                return vec![];
            }
        };

        let rows = stmt.query_map(rusqlite::params![start_ts, end_ts], |row| {
            let model: Option<String> = row.get(0)?;
            let input_tokens: i64 = row.get(1)?;
            let output_tokens: i64 = row.get(2)?;
            let cache_read: i64 = row.get(3)?;
            let cache_write: i64 = row.get(4)?;
            let reasoning: i64 = row.get(5)?;
            let started_at: f64 = row.get(6)?;
            let est_cost: Option<f64> = row.get(7)?;
            let act_cost: Option<f64> = row.get(8)?;
            let source: Option<String> = row.get(9)?;

            let dt = chrono::DateTime::from_timestamp(started_at as i64, 0)
                .map(|d| d.naive_utc())
                .unwrap_or_default();
            let cost = act_cost.unwrap_or(est_cost.unwrap_or(0.0));
            let src = match source.as_deref() {
                Some("cli") => SourceType::Cli,
                Some("gui") | Some("desktop") => SourceType::Gui,
                _ => SourceType::Unknown,
            };

            Ok(UsageRecord {
                cli_name: "hermes".to_string(),
                source_type: src,
                model_name: model.unwrap_or_else(|| "unknown".to_string()),
                prompt_tokens: input_tokens.max(0) as u64,
                completion_tokens: output_tokens.max(0) as u64,
                total_tokens: (input_tokens + output_tokens).max(0) as u64,
                cache_read_tokens: (cache_read.max(0) as u64).min(input_tokens.max(0) as u64),
                cache_write_tokens: cache_write.max(0) as u64,
                reasoning_tokens: reasoning.max(0) as u64,
                request_count: 1,
                cost_cents: (cost * 100.0) as u64,
                date: dt.date(),
                hour: dt.hour() as u8,
                timestamp: dt,
            })
        });

        let mut records = Vec::new();
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                records.push(row);
            }
        }

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 1.0,
            message: format!("Done: {} records, {} tokens", records.len(), format_tokens_full(records.iter().map(|r| r.total_tokens).sum())),
            done: true, error: None,
        });
        records
    }
}

// ─── codex ────────────────────────────────────────────────────

pub struct CodexReader;

impl UsageReader for CodexReader {
    fn name(&self) -> &str { "codex" }
    fn is_installed(&self) -> bool {
        which_installed("codex") || home_dir().join(".codex/state_5.sqlite").exists()
    }
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord> {
        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.05, message: "Opening database...".into(), done: false, error: None,
        });

        let db_path = home_dir().join(".codex/state_5.sqlite");
        if !db_path.exists() {
            let _ = progress.send(ProgressUpdate {
                cli_name: self.name().to_string(), percent: 1.0, message: "Database not found".into(), done: true, error: Some("Database not found".into()),
            });
            return vec![];
        }

        let conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: 1.0, message: format!("Error: {e}"), done: true, error: Some(e.to_string()),
                });
                return vec![];
            }
        };

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.2, message: "Querying threads...".into(), done: false, error: None,
        });

        let start_ts = range.start.and_hms_opt(0, 0, 0).map(|d| d.and_utc().timestamp()).unwrap_or(0);
        let end_ts = range.end.and_hms_opt(23, 59, 59).map(|d| d.and_utc().timestamp()).unwrap_or(0);

        // Main query: threads + optional JSONL cache data
        let mut stmt = match conn.prepare(
            "SELECT model, tokens_used, source, created_at, rollout_path
             FROM threads
             WHERE created_at >= ?1 AND created_at <= ?2
             ORDER BY created_at"
        ) {
            Ok(s) => s,
            Err(e) => {
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: 1.0, message: format!("Query error: {e}"), done: true, error: Some(e.to_string()),
                });
                return vec![];
            }
        };

        struct ThreadRow {
            model: Option<String>,
            tokens_used: i64,
            source: Option<String>,
            created_at: i64,
            rollout_path: Option<String>,
        }

        let rows: Vec<ThreadRow> = stmt.query_map(rusqlite::params![start_ts, end_ts], |row| {
            Ok(ThreadRow {
                model: row.get(0)?,
                tokens_used: row.get(1)?,
                source: row.get(2)?,
                created_at: row.get(3)?,
                rollout_path: row.get(4)?,
            })
        })
        .map(|r| r.flatten().collect())
        .unwrap_or_default();

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.4,
            message: format!("Found {} threads, parsing cache data...", rows.len()), done: false, error: None,
        });

        // Try to read cache data from JSONL files
        let total = rows.len();
        let mut records: Vec<UsageRecord> = Vec::new();

        let codex_dir = home_dir().join(".codex");
        for (i, thread) in rows.into_iter().enumerate() {
            let dt = chrono::DateTime::from_timestamp(thread.created_at, 0)
                .map(|d| d.naive_utc())
                .unwrap_or_default();
            let src = match thread.source.as_deref() {
                Some("cli") => SourceType::Cli,
                Some("vscode") => SourceType::Gui,
                _ => SourceType::Unknown,
            };

            let mut cache_read: u64 = 0;
            let cache_write: u64 = 0;

            // Try to find cache data from JSONL rollout file
            if let Some(ref path) = thread.rollout_path {
                let jsonl_path = codex_dir.join(path);
                if jsonl_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&jsonl_path) {
                        // Scan first 20 lines for usage metadata with cache info
                        for line in content.lines().take(20) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                                if let Some(usage) = val.get("usage") {
                                    if let Some(details) = usage.get("input_token_details") {
                                        cache_read += details.get("cached_tokens").and_then(|c| c.as_u64()).unwrap_or(0);
                                    }
                                }
                                if let Some(meta) = val.get("usageMetadata") {
                                    cache_read += meta.get("cachedContentTokenCount").and_then(|c| c.as_u64()).unwrap_or(0);
                                }
                            }
                        }
                    }
                }
            }

            records.push(UsageRecord {
                cli_name: "codex".to_string(),
                source_type: src,
                model_name: thread.model.unwrap_or_else(|| "unknown".to_string()),
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: thread.tokens_used.max(0) as u64,
                cache_read_tokens: cache_read,
                cache_write_tokens: cache_write,
                reasoning_tokens: 0,
                request_count: 1,
                cost_cents: 0,
                date: dt.date(),
                hour: dt.hour() as u8,
                timestamp: dt,
            });

            if i % 5 == 0 {
                let p = 0.4 + (i as f32 / total.max(1) as f32) * 0.6;
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: p,
                    message: format!("Parsed {}/{} threads", i + 1, total), done: false, error: None,
                });
            }
        }

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 1.0,
            message: format!("Done: {} threads, {} tokens", records.len(), format_tokens_full(records.iter().map(|r| r.total_tokens).sum())),
            done: true, error: None,
        });
        records
    }
}

// ─── claude ────────────────────────────────────────────────────

pub struct ClaudeReader;

impl UsageReader for ClaudeReader {
    fn name(&self) -> &str { "claude" }
    fn is_installed(&self) -> bool {
        which_installed("claude") || home_dir().join(".claude/stats-cache.json").exists()
    }
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord> {
        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.1, message: "Scanning data...".into(), done: false, error: None,
        });

        let claude_dir = home_dir().join(".claude");
        let mut records: Vec<UsageRecord> = Vec::new();

        // Method 1: Read stats-cache.json (fast, has model-level data with cache info)
        let cache_path = claude_dir.join("stats-cache.json");
        let has_stats_cache = cache_path.exists();

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.2,
            message: if has_stats_cache { "Reading stats-cache.json...".into() } else { "No stats-cache.json found".into() },
            done: false, error: None,
        });

        let mut daily_by_model: Vec<(NaiveDate, String, u64)> = Vec::new();
        let mut per_model_totals: std::collections::HashMap<String, (u64, u64, u64, u64)> = std::collections::HashMap::new();

        if has_stats_cache {
            if let Ok(content) = std::fs::read_to_string(&cache_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Read modelUsage per-model aggregates (has cache info)
                    if let Some(mu) = json.get("modelUsage").and_then(|v| v.as_object()) {
                        for (model, data) in mu {
                            let inp = data.get("inputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let out = data.get("outputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let cache_r = data.get("cacheReadInputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let cache_c = data.get("cacheCreationInputTokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            per_model_totals.insert(model.clone(), (inp, out, cache_r, cache_c));
                        }
                    }

                    // Read dailyModelTokens for date-range filtering
                    if let Some(daily) = json.get("dailyModelTokens").and_then(|v| v.as_array()) {
                        for entry in daily {
                            let date_str = entry.get("date").and_then(|v| v.as_str()).unwrap_or("");
                            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                                if range.contains(date) {
                                    if let Some(tbm) = entry.get("tokensByModel").and_then(|v| v.as_object()) {
                                        for (model, tokens) in tbm {
                                            let t = tokens.as_u64().unwrap_or(0);
                                            daily_by_model.push((date, model.clone(), t));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Method 2: Read session-meta JSON files (per-session input/output)
        let meta_dir = claude_dir.join("usage-data/session-meta");
        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.5, message: "Reading session-meta files...".into(), done: false, error: None,
        });

        let mut session_entries: Vec<(NaiveDate, u8, u64, u64, String)> = Vec::new();

        if meta_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&meta_dir) {
                let files: Vec<_> = entries.filter_map(|e| e.ok()).filter(|e| {
                    e.path().extension().and_then(|s| s.to_str()) == Some("json")
                }).collect();

                let total_files = files.len();
                for (i, entry) in files.iter().enumerate() {
                    let path = entry.path();
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            let start_time = json.get("start_time").and_then(|v| v.as_str()).unwrap_or("");
                            let inp = json.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                            let out = json.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

                            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(start_time) {
                                let date = dt.date_naive();
                                if range.contains(date) {
                                    // Try to get model from session JSON (not always present)
                                    let model = json.get("model")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| "unknown".to_string());
                                    session_entries.push((date, dt.hour() as u8, inp, out, model));
                                }
                            }
                        }
                    }

                    if i % 10 == 0 {
                        let p = 0.5 + (i as f32 / total_files.max(1) as f32) * 0.5;
                        let _ = progress.send(ProgressUpdate {
                            cli_name: self.name().to_string(), percent: p,
                            message: format!("Reading session files {}/{}", i + 1, total_files), done: false, error: None,
                        });
                    }
                }
            }
        }

        // Build records from session entries
        for (date, hour, inp, out, model_name) in session_entries {
            let model = if model_name == "unknown" {
                // Try to find model from per_model_totals
                if !per_model_totals.is_empty() {
                    // Use the model with highest token count for approximate attribution
                    per_model_totals.iter()
                        .max_by_key(|(_, (i, o, _, _))| *i + *o)
                        .map(|(name, _)| name.clone())
                        .unwrap_or_else(|| "unknown".to_string())
                } else {
                    "unknown".to_string()
                }
            } else {
                model_name.clone()
            };

            let (_, _, cache_r, cache_w) = per_model_totals.get(&model).copied().unwrap_or((0, 0, 0, 0));

            records.push(UsageRecord {
                cli_name: "claude".to_string(),
                source_type: SourceType::Cli,
                model_name: model,
                prompt_tokens: inp,
                completion_tokens: out,
                total_tokens: inp + out,
                cache_read_tokens: cache_r.min(inp),
                cache_write_tokens: cache_w,
                reasoning_tokens: 0,
                request_count: 1,
                cost_cents: 0,
                date,
                hour,
                timestamp: date.and_hms_opt(hour as u32, 0, 0).unwrap_or_default(),
            });
        }

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 1.0,
            message: format!("Done: {} records", records.len()), done: true, error: None,
        });
        records
    }
}

// ─── qwen ──────────────────────────────────────────────────────

pub struct QwenReader;

impl UsageReader for QwenReader {
    fn name(&self) -> &str { "qwen" }
    fn is_installed(&self) -> bool {
        which_installed("qwen") || home_dir().join(".qwen/settings.json").exists()
    }
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord> {
        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.1, message: "Scanning project chat files...".into(), done: false, error: None,
        });

        let qwen_dir = home_dir().join(".qwen/projects");
        if !qwen_dir.exists() {
            let _ = progress.send(ProgressUpdate {
                cli_name: self.name().to_string(), percent: 1.0, message: "No qwen data found".into(), done: true, error: None,
            });
            return vec![];
        }

        // Collect all JSONL chat files
        let mut chat_files: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&qwen_dir) {
            for entry in entries.flatten() {
                let chats_dir = entry.path().join("chats");
                if chats_dir.exists() {
                    if let Ok(chat_entries) = std::fs::read_dir(&chats_dir) {
                        for chat in chat_entries.flatten() {
                            let path = chat.path();
                            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                                chat_files.push(path);
                            }
                        }
                    }
                }
            }
        }

        let total_files = chat_files.len();
        if total_files == 0 {
            let _ = progress.send(ProgressUpdate {
                cli_name: self.name().to_string(), percent: 1.0, message: "No chat files found".into(), done: true, error: None,
            });
            return vec![];
        }

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 0.15,
            message: format!("Found {total_files} chat files, parsing..."), done: false, error: None,
        });

        let mut records: Vec<UsageRecord> = Vec::new();

        for (i, path) in chat_files.iter().enumerate() {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    if line.trim().is_empty() { continue; }
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

                        let (prompt, output, cached, total, model, ts_str) = if msg_type == "assistant" {
                            // From usageMetadata
                            let usage = val.get("usageMetadata");
                            (
                                usage.and_then(|u| u.get("promptTokenCount")).and_then(|v| v.as_u64()).unwrap_or(0),
                                usage.and_then(|u| u.get("candidatesTokenCount")).and_then(|v| v.as_u64()).unwrap_or(0),
                                usage.and_then(|u| u.get("cachedContentTokenCount")).and_then(|v| v.as_u64()).unwrap_or(0),
                                usage.and_then(|u| u.get("totalTokenCount")).and_then(|v| v.as_u64()).unwrap_or(0),
                                val.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                                val.get("timestamp").or_else(|| val.get("time")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            )
                        } else if msg_type == "system" {
                            // From ui_telemetry events
                            let payload = val.get("systemPayload")
                                .and_then(|p| p.get("uiEvent"));
                            let event = payload.and_then(|p| p.get("event.name")).and_then(|v| v.as_str());
                            if event == Some("qwen-code.api_response") {
                                (
                                    payload.and_then(|p| p.get("input_token_count")).and_then(|v| v.as_u64()).unwrap_or(0),
                                    payload.and_then(|p| p.get("output_token_count")).and_then(|v| v.as_u64()).unwrap_or(0),
                                    payload.and_then(|p| p.get("cached_content_token_count")).and_then(|v| v.as_u64()).unwrap_or(0),
                                    payload.and_then(|p| p.get("total_token_count")).and_then(|v| v.as_u64()).unwrap_or(0),
                                    payload.and_then(|p| p.get("model")).and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                                    val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                )
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        };

                        let dt = if let Ok(d) = chrono::DateTime::parse_from_rfc3339(&ts_str) {
                            d
                        } else if let Ok(d) = chrono::DateTime::parse_from_rfc3339(&format!("{}T00:00:00Z", &ts_str[..10])) {
                            d
                        } else {
                            continue;
                        };

                        let date = dt.date_naive();
                        if !range.contains(date) {
                            continue;
                        }

                        records.push(UsageRecord {
                            cli_name: "qwen".to_string(),
                            source_type: SourceType::Cli,
                            model_name: model,
                            prompt_tokens: prompt,
                            completion_tokens: output,
                            total_tokens: if total > 0 { total } else { prompt + output },
                            cache_read_tokens: cached.min(prompt),
                            cache_write_tokens: 0,
                            reasoning_tokens: 0,
                            request_count: 1,
                            cost_cents: 0,
                            date,
                            hour: dt.hour() as u8,
                            timestamp: dt.naive_local(),
                        });
                    }
                }
            }

            if i % 5 == 0 || i == total_files - 1 {
                let p = 0.15 + (i as f32 / total_files.max(1) as f32) * 0.85;
                let _ = progress.send(ProgressUpdate {
                    cli_name: self.name().to_string(), percent: p,
                    message: format!("Parsed {}/{} chat files", i + 1, total_files), done: false, error: None,
                });
            }
        }

        let _ = progress.send(ProgressUpdate {
            cli_name: self.name().to_string(), percent: 1.0,
            message: format!("Done: {} records, {} tokens", records.len(), format_tokens_full(records.iter().map(|r| r.total_tokens).sum())),
            done: true, error: None,
        });
        records
    }
}

// ─── helpers ──────────────────────────────────────────────────

fn which_installed(name: &str) -> bool {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|dir| std::path::Path::new(dir).join(name).exists())
}

pub fn detect_readers() -> Vec<Box<dyn UsageReader>> {
    let readers: Vec<Box<dyn UsageReader>> = vec![
        Box::new(OpencodeReader),
        Box::new(HermesReader),
        Box::new(CodexReader),
        Box::new(ClaudeReader),
        Box::new(QwenReader),
    ];

    let installed: Vec<_> = readers.into_iter()
        .filter(|r| r.is_installed())
        .collect();

    if installed.is_empty() {
        tracing::warn!("No supported CLI tools detected!");
    } else {
        tracing::info!("Detected CLI tools: {:?}", installed.iter().map(|r| r.name()).collect::<Vec<_>>());
    }

    installed
}
