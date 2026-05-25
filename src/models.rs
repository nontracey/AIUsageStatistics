use chrono::{Datelike, NaiveDate, NaiveDateTime};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UsageRecord {
    pub session_id: String,
    pub cli_name: String,
    pub source_type: SourceType,
    pub model_name: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
    pub request_count: u64,
    pub cost_cents: u64,
    pub date: NaiveDate,
    pub hour: u8,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SourceType {
    Cli,
    Gui,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub cli_name: String,
    pub percent: f32,
    pub message: String,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReaderResult {
    pub cli_name: String,
    pub records: Vec<UsageRecord>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DateRangeType {
    Today,
    Week,
    Month,
    Custom,
}

#[derive(Debug, Clone)]
pub struct DateRange {
    pub range_type: DateRangeType,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl DateRange {
    pub fn today() -> Self {
        let now = chrono::Local::now().date_naive();
        Self {
            range_type: DateRangeType::Today,
            start: now,
            end: now,
        }
    }

    pub fn this_week() -> Self {
        let now = chrono::Local::now().date_naive();
        let weekday = now.weekday().num_days_from_monday();
        let monday = now - chrono::Duration::days(weekday as i64);
        Self {
            range_type: DateRangeType::Week,
            start: monday,
            end: now,
        }
    }

    pub fn this_month() -> Self {
        let now = chrono::Local::now().date_naive();
        let first = NaiveDate::from_ymd_opt(now.year(), now.month(), 1).unwrap();
        Self {
            range_type: DateRangeType::Month,
            start: first,
            end: now,
        }
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

#[derive(Debug, Clone)]
pub struct CliSummary {
    pub total_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct ModelBreakdown {
    pub cli_name: String,
    pub model_name: String,
    pub total_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub reasoning_tokens: u64,
    pub request_count: u64,
    pub cost_cents: u64,
}

#[derive(Debug, Clone)]
pub struct HourlyPoint {
    pub hour: u8,
    pub total_tokens: u64,
    pub cache_hit: u64,
}

pub fn aggregate_by_cli(records: &[UsageRecord]) -> Vec<CliSummary> {
    let mut by_cli: HashMap<String, Vec<&UsageRecord>> = HashMap::new();
    for r in records {
        by_cli.entry(r.cli_name.clone()).or_default().push(r);
    }

    let mut result: Vec<CliSummary> = by_cli
        .into_iter()
        .map(|(_, recs)| CliSummary {
            total_tokens: recs.iter().map(|r| r.total_tokens).sum(),
        })
        .collect();

    result.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    result
}

pub fn aggregate_by_model(records: &[UsageRecord]) -> Vec<ModelBreakdown> {
    let mut by_model: HashMap<(String, String), ModelBreakdown> = HashMap::new();

    for r in records {
        let key = (r.cli_name.clone(), r.model_name.clone());
        let entry = by_model.entry(key).or_insert(ModelBreakdown {
            cli_name: r.cli_name.clone(),
            model_name: r.model_name.clone(),
            total_tokens: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            cache_read: 0,
            cache_write: 0,
            reasoning_tokens: 0,
            request_count: 0,
            cost_cents: 0,
        });
        entry.total_tokens += r.total_tokens;
        entry.prompt_tokens += r.prompt_tokens;
        entry.completion_tokens += r.completion_tokens;
        entry.cache_read += r.cache_read_tokens;
        entry.cache_write += r.cache_write_tokens;
        entry.reasoning_tokens += r.reasoning_tokens;
        entry.request_count += r.request_count;
        entry.cost_cents += r.cost_cents;
    }

    let mut result: Vec<ModelBreakdown> = by_model.into_values().collect();
    result.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
    result
}

pub fn aggregate_hourly(records: &[UsageRecord]) -> Vec<HourlyPoint> {
    let mut by_hour: HashMap<u8, (u64, u64)> = HashMap::new();
    for r in records {
        let entry = by_hour.entry(r.hour).or_insert((0, 0));
        entry.0 += r.total_tokens;
        entry.1 += r.cache_read_tokens;
    }

    let mut result: Vec<HourlyPoint> = by_hour
        .into_iter()
        .map(|(hour, (total, cache))| HourlyPoint {
            hour,
            total_tokens: total,
            cache_hit: cache,
        })
        .collect();
    result.sort_by_key(|p| p.hour);
    result
}

pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn format_tokens_full(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, c);
    }
    result
}
