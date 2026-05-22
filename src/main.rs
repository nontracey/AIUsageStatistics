#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod models;
mod readers;
mod updater;

use models::*;
use readers::*;
use updater::*;

use chrono::{NaiveDate, Local, Datelike};
use eframe::egui::{self, Color32, FontFamily, FontId, CornerRadius, Vec2, Margin, Align2, Frame, Sense};
use egui_plot::{Line, Plot, PlotPoints};
use image::GenericImageView;
use std::collections::{HashMap, BTreeSet};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

// ─── Theme Colors ───────────────────────────────────────────────

struct ThemeColors {
    accent: Color32,
    accent_light: Color32,
    surface: Color32,
    surface2: Color32,
    surface3: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    green: Color32,
    cyan: Color32,
    yellow: Color32,
    orange: Color32,
}

const DARK: ThemeColors = ThemeColors {
    accent: Color32::from_rgb(0x7c, 0x3a, 0xed),
    accent_light: Color32::from_rgb(0xa7, 0x8b, 0xf0),
    surface: Color32::from_rgb(0x1e, 0x1e, 0x2e),
    surface2: Color32::from_rgb(0x2a, 0x2a, 0x3e),
    surface3: Color32::from_rgb(0x33, 0x33, 0x4e),
    text_primary: Color32::from_rgb(0xee, 0xee, 0xff),
    text_secondary: Color32::from_rgb(0x99, 0x99, 0xbb),
    green: Color32::from_rgb(0x00, 0xb8, 0x94),
    cyan: Color32::from_rgb(0x06, 0xb6, 0xd4),
    yellow: Color32::from_rgb(0xe8, 0xbf, 0x3e),
    orange: Color32::from_rgb(0xf6, 0x8f, 0x3e),
};

const LIGHT: ThemeColors = ThemeColors {
    accent: Color32::from_rgb(0x7c, 0x3a, 0xed),
    accent_light: Color32::from_rgb(0x9d, 0x7e, 0xf0),
    surface: Color32::from_rgb(0xf8, 0xf8, 0xfc),
    surface2: Color32::from_rgb(0xee, 0xee, 0xf4),
    surface3: Color32::from_rgb(0xe2, 0xe2, 0xea),
    text_primary: Color32::from_rgb(0x1a, 0x1a, 0x2e),
    text_secondary: Color32::from_rgb(0x66, 0x66, 0x88),
    green: Color32::from_rgb(0x00, 0x8a, 0x6e),
    cyan: Color32::from_rgb(0x05, 0x86, 0xa0),
    yellow: Color32::from_rgb(0xb0, 0x8f, 0x1e),
    orange: Color32::from_rgb(0xc6, 0x68, 0x1e),
};

const CLI_COLORS: &[Color32] = &[
    Color32::from_rgb(0x7c, 0x3a, 0xed),
    Color32::from_rgb(0x06, 0xb6, 0xd4),
    Color32::from_rgb(0x00, 0xb8, 0x94),
    Color32::from_rgb(0xe8, 0xbf, 0x3e),
    Color32::from_rgb(0xf6, 0x8f, 0x3e),
];

fn cli_color(name: &str, alpha: u8) -> Color32 {
    let idx = match name {
        "opencode" => 0,
        "hermes" => 1,
        "codex" => 2,
        "claude" => 3,
        "qwen" => 4,
        _ => 0,
    };
    let c = CLI_COLORS[idx % CLI_COLORS.len()];
    Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), alpha)
}

fn load_tool_icon(ctx: &egui::Context, name: &str) -> Option<egui::TextureHandle> {
    let bytes: &[u8] = match name {
        "opencode" => include_bytes!("../icons/opencode.png"),
        "hermes" => include_bytes!("../icons/hermes.png"),
        "codex" => include_bytes!("../icons/codex.png"),
        "claude" => include_bytes!("../icons/claude.png"),
        "qwen" => include_bytes!("../icons/qwen.png"),
        _ => return None,
    };
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = img.dimensions();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR))
}

fn tool_paths() -> Vec<(&'static str, String)> {
    let home = dirs::home_dir().unwrap_or_default();
    let home_str = home.to_string_lossy().to_string();
    vec![
        ("opencode", format!("{}/.local/share/opencode/opencode.db", home_str)),
        ("hermes", format!("{}/.hermes/state.db", home_str)),
        ("codex", format!("{}/.codex/state_5.sqlite", home_str)),
        ("claude", format!("{}/.claude", home_str)),
        ("qwen", format!("{}/.qwen", home_str)),
    ]
}

// ─── Language / i18n ───────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum Language { Chinese, English }

impl Language {
    fn label(self) -> &'static str {
        match self {
            Language::Chinese => "中文",
            Language::English => "English",
        }
    }
}

fn tr(lang: Language, key: &str) -> &'static str {
    match (lang, key) {
        (Language::Chinese, "title") => "\u{1F4CA} AI \u{7528}\u{91CF}\u{7EDF}\u{8BA1}",
        (Language::English, "title") => "\u{1F4CA} AI Usage Statistics",
        (Language::Chinese, "refresh") => "\u{1F504} \u{5237}\u{65B0}",
        (Language::English, "refresh") => "\u{21BB} Refresh",
        (Language::Chinese, "summary_tab") => "\u{1F4C8} \u{6982}\u{89C8}",
        (Language::English, "summary_tab") => "\u{1F4C8} Summary",
        (Language::Chinese, "loading_title") => "\u{1F4E5} \u{6B63}\u{5728}\u{52A0}\u{8F7D}\u{7528}\u{91CF}\u{6570}\u{636E}",
        (Language::English, "loading_title") => "\u{1F4E5} Loading Usage Data",
        (Language::Chinese, "no_data_today") => "\u{1F4ED} \u{4ECA}\u{5929}\u{6CA1}\u{6709}\u{7528}\u{91CF}\u{6570}\u{636E}",
        (Language::English, "no_data_today") => "\u{1F4ED} No usage data found for today.",
        (Language::Chinese, "try_change_range") => "\u{8BD5}\u{8BD5}\u{5207}\u{6362}\u{65F6}\u{95F4}\u{8303}\u{56F4}\u{FF0C}\u{6216}\u{5148}\u{4F7F}\u{7528} AI \u{5DE5}\u{5177}",
        (Language::English, "try_change_range") => "Try changing the time range, or use an AI tool first.",
        (Language::Chinese, "no_data") => "\u{6682}\u{65E0}\u{6570}\u{636E}",
        (Language::English, "no_data") => "No data",
        // Stat cards
        (Language::Chinese, "total_tokens") => "\u{603B} Token \u{6570}",
        (Language::English, "total_tokens") => "Total Tokens",
        (Language::Chinese, "cache_hit_rate") => "\u{7F13}\u{5B58}\u{547D}\u{4E2D}\u{7387}",
        (Language::English, "cache_hit_rate") => "Cache Hit Rate",
        (Language::Chinese, "cached") => "\u{5DF2}\u{7F13}\u{5B58}",
        (Language::English, "cached") => "cached",
        (Language::Chinese, "total_requests") => "\u{603B}\u{8BF7}\u{6C42}\u{6570}",
        (Language::English, "total_requests") => "Total Requests",
        (Language::Chinese, "active_tools") => "\u{6D3B}\u{8DC3}\u{5DE5}\u{5177}",
        (Language::English, "active_tools") => "Active Tools",
        (Language::Chinese, "sessions") => "\u{6B21}\u{4F1A}\u{8BDD}",
        (Language::English, "sessions") => "sessions",
        // Table headers
        (Language::Chinese, "tool") => "\u{5DE5}\u{5177}",
        (Language::English, "tool") => "Tool",
        (Language::Chinese, "model") => "\u{6A21}\u{578B}",
        (Language::English, "model") => "Model",
        (Language::Chinese, "requests") => "\u{8BF7}\u{6C42}\u{6570}",
        (Language::English, "requests") => "Requests",
        (Language::Chinese, "tokens") => "Token \u{6570}",
        (Language::English, "tokens") => "Tokens",
        (Language::Chinese, "input") => "\u{8F93}\u{5165}",
        (Language::English, "input") => "Input",
        (Language::Chinese, "output") => "\u{8F93}\u{51FA}",
        (Language::English, "output") => "Output",
        (Language::Chinese, "cache") => "\u{7F13}\u{5B58}",
        (Language::English, "cache") => "Cache",
        (Language::Chinese, "cache_pct") => "\u{7F13}\u{5B58}%",
        (Language::English, "cache_pct") => "Cache %",
        (Language::Chinese, "cost") => "\u{8D39}\u{7528}",
        (Language::English, "cost") => "Cost",
        // Sections
        (Language::Chinese, "summary_stats") => "\u{6570}\u{636E}\u{6982}\u{89C8}",
        (Language::English, "summary_stats") => "Summary Statistics",
        (Language::Chinese, "tool_breakdown") => "\u{5DE5}\u{5177}\u{660E}\u{7EC6}",
        (Language::English, "tool_breakdown") => "Tool Breakdown",
        (Language::Chinese, "model_breakdown") => "\u{6A21}\u{578B}\u{7528}\u{91CF}\u{660E}\u{7EC6}",
        (Language::English, "model_breakdown") => "Model Usage Breakdown",
        (Language::Chinese, "hourly_chart") => "\u{5C0F}\u{65F6}\u{7EA7} Token \u{8D8B}\u{52BF}",
        (Language::English, "hourly_chart") => "Hourly Token Usage Trend",
        // Miscellaneous
        (Language::Chinese, "loading") => "\u{52A0}\u{8F7D}\u{4E2D}...",
        (Language::English, "loading") => "Loading...",
        (Language::Chinese, "no_period_data") => "\u{8BE5}\u{65F6}\u{95F4}\u{6BB5}\u{5185}\u{6CA1}\u{6709}\u{4F7F}\u{7528}\u{8BB0}\u{5F55}",
        (Language::English, "no_period_data") => "No usage records for this period.",
        // Date range
        (Language::Chinese, "today") => "\u{4ECA}\u{5929}",
        (Language::English, "today") => "Today",
        (Language::Chinese, "week") => "\u{672C}\u{5468}",
        (Language::English, "week") => "Week",
        (Language::Chinese, "month") => "\u{672C}\u{6708}",
        (Language::English, "month") => "Month",
        (Language::Chinese, "custom") => "\u{81EA}\u{5B9A}\u{4E49}",
        (Language::English, "custom") => "Custom",
        (Language::Chinese, "apply") => "\u{5E94}\u{7528}",
        (Language::English, "apply") => "Apply",
        // Settings
        (Language::Chinese, "settings") => "\u{8BBE}\u{7F6E}",
        (Language::English, "settings") => "Settings",
        (Language::Chinese, "theme") => "\u{4E3B}\u{9898}",
        (Language::English, "theme") => "Theme",
        (Language::Chinese, "theme_dark") => "\u{6DF1}\u{8272}",
        (Language::English, "theme_dark") => "Dark",
        (Language::Chinese, "theme_light") => "\u{6D45}\u{8272}",
        (Language::English, "theme_light") => "Light",
        (Language::Chinese, "language") => "\u{8BED}\u{8A00}",
        (Language::English, "language") => "Language",
        (Language::Chinese, "tool_paths") => "\u{5DE5}\u{5177}\u{8DEF}\u{5F84}",
        (Language::English, "tool_paths") => "Tool Paths",
        (Language::Chinese, "tool_order_label") => "\u{5DE5}\u{5177}\u{987A}\u{5E8F}",
        (Language::English, "tool_order_label") => "Tool Order",
        (Language::Chinese, "version") => "\u{7248}\u{672C}",
        (Language::English, "version") => "Version",
        (Language::Chinese, "close") => "\u{5173}\u{95ED}",
        (Language::English, "close") => "Close",
        (Language::Chinese, "waiting") => "\u{7B49}\u{5F85}\u{4E2D}...",
        (Language::English, "waiting") => "Waiting...",
        _ => "",
    }
}

// ─── Theme ──────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum Theme { Dark, Light }

impl Theme {
    fn colors(self) -> &'static ThemeColors {
        match self {
            Theme::Dark => &DARK,
            Theme::Light => &LIGHT,
        }
    }
}

// ─── Settings & Persistence ────────────────────────────────────

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct AppSettings {
    theme: String,
    language: String,
    tool_order: Vec<String>,
    tool_path_overrides: HashMap<String, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "Dark".into(),
            language: "Chinese".into(),
            tool_order: vec![],
            tool_path_overrides: HashMap::new(),
        }
    }
}

fn config_dir() -> std::path::PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"));
    base.join("ai-usage-stats")
}

fn save_config(settings: &AppSettings) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("config.json");
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(&path, json);
    }
}

fn load_config() -> AppSettings {
    let path = config_dir().join("config.json");
    if let Ok(json) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str(&json) {
            return s;
        }
    }
    AppSettings::default()
}

// ─── Tab ─────────────────────────────────────────────────────────

#[derive(PartialEq, Clone)]
enum Tab {
    Summary,
    Tool(String),
}

// ─── Date Picker State ──────────────────────────────────────────

struct DatePickerState {
    open: bool,
    target: String,
    year: i32,
    month: u32,
}

impl DatePickerState {
    fn new() -> Self {
        let now = Local::now();
        Self { open: false, target: String::new(), year: now.year(), month: now.month() }
    }
}

// ─── Main App ────────────────────────────────────────────────────

pub struct AiUsageApp {
    loading: bool,
    load_started: bool,
    progress_updates: HashMap<String, ProgressUpdate>,
    all_records: Vec<UsageRecord>,
    records_by_cli: HashMap<String, Vec<UsageRecord>>,
    progress_rx: mpsc::Receiver<ProgressUpdate>,
    result_rx: mpsc::Receiver<ReaderResult>,
    detected_clis: Vec<String>,
    load_errors: Vec<String>,

    current_tab: Tab,
    date_range: DateRange,
    custom_start: NaiveDate,
    custom_end: NaiveDate,
    date_picker: DatePickerState,

    cli_summaries: Vec<CliSummary>,
    model_breakdowns: Vec<ModelBreakdown>,
    hourly_data: Vec<HourlyPoint>,
    summary_total_tokens: u64,
    summary_total_cache: u64,
    summary_total_requests: u64,
    models_cache: HashMap<String, String>,

    load_start_time: Option<Instant>,
    last_refresh: Instant,

    show_settings: bool,
    theme: Theme,
    language: Language,
    tool_order: Vec<String>,
    tool_path_overrides: HashMap<String, String>,

    tool_textures: HashMap<String, egui::TextureHandle>,

    update_status: String,
    update_info: Option<UpdateInfo>,
    update_downloaded: Option<std::path::PathBuf>,
    update_rx: mpsc::Receiver<String>,
}

enum UpdateAction {
    Check,
    Download,
}

fn spawn_update(action: UpdateAction, url: String, asset_name: String) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = match action {
            UpdateAction::Check => {
                match check_for_update(env!("CARGO_PKG_VERSION")) {
                    Ok(Some(info)) => format!("found:{}:{}:{}", info.version, info.download_url, info.asset_name),
                    Ok(None) => "uptodate".into(),
                    Err(e) => format!("error:{}", e),
                }
            }
            UpdateAction::Download => {
                let info = UpdateInfo {
                    version: String::new(),
                    download_url: url,
                    asset_name,
                };
                match download_and_install(&info) {
                    Ok(path) => format!("dl_ok:{}", path.display()),
                    Err(e) => format!("dl_err:{}", e),
                }
            }
        };
        let _ = tx.send(result);
    });
    rx
}

impl Default for AiUsageApp {
    fn default() -> Self {
        let config = load_config();

        let dr = DateRange::today();
        let (progress_tx, progress_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();

        let readers = detect_readers();
        let detected_names: Vec<String> = readers.iter().map(|r| r.name().to_string()).collect();
        let reader_count = readers.len();

        if reader_count > 0 {
            let range = dr.clone();
            for reader in readers {
                let pt = progress_tx.clone();
                let rt = result_tx.clone();
                let rng = range.clone();
                thread::spawn(move || {
                    let records = reader.read(&rng, &pt);
                    let _ = rt.send(ReaderResult {
                        cli_name: reader.name().to_string(),
                        records,
                        error: None,
                    });
                });
            }
        }

        let theme = match config.theme.as_str() {
            "Light" => Theme::Light,
            _ => Theme::Dark,
        };
        let language = match config.language.as_str() {
            "English" => Language::English,
            _ => Language::Chinese,
        };

        Self {
            loading: reader_count > 0,
            load_started: true,
            progress_updates: HashMap::new(),
            all_records: Vec::new(),
            records_by_cli: HashMap::new(),
            progress_rx,
            result_rx,
            detected_clis: detected_names.clone(),
            load_errors: Vec::new(),

            current_tab: Tab::Summary,
            date_range: dr.clone(),
            custom_start: Local::now().date_naive(),
            custom_end: Local::now().date_naive(),
            date_picker: DatePickerState::new(),

            cli_summaries: Vec::new(),
            model_breakdowns: Vec::new(),
            hourly_data: Vec::new(),
            summary_total_tokens: 0,
            summary_total_cache: 0,
            summary_total_requests: 0,
            models_cache: HashMap::new(),

            load_start_time: if reader_count > 0 { Some(Instant::now()) } else { None },
            last_refresh: Instant::now(),

            show_settings: false,
            theme,
            language,
            tool_order: config.tool_order.clone(),
            tool_path_overrides: config.tool_path_overrides.clone(),

            tool_textures: HashMap::new(),

            update_status: String::new(),
            update_info: None,
            update_downloaded: None,
            update_rx: mpsc::channel().1,
        }
    }
}

impl AiUsageApp {
    fn tc(&self) -> &'static ThemeColors {
        self.theme.colors()
    }

    fn tr(&self, key: &str) -> &'static str {
        tr(self.language, key)
    }

    fn save_config(&self) {
        let config = AppSettings {
            theme: match self.theme { Theme::Dark => "Dark".into(), Theme::Light => "Light".into() },
            language: match self.language { Language::Chinese => "Chinese".into(), Language::English => "English".into() },
            tool_order: self.tool_order.clone(),
            tool_path_overrides: self.tool_path_overrides.clone(),
        };
        save_config(&config);
    }

    fn init_textures(&mut self, ctx: &egui::Context) {
        for name in &self.detected_clis {
            if !self.tool_textures.contains_key(name) {
                if let Some(handle) = load_tool_icon(ctx, name) {
                    self.tool_textures.insert(name.to_string(), handle);
                }
            }
        }
    }

    fn tool_avatar(&self, ui: &mut egui::Ui, name: &str, size: f32) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(size, size), Sense::hover());
        let Some(tex) = self.tool_textures.get(name) else {
            let color = cli_color(name, 255);
            ui.painter().circle_filled(rect.center(), size / 2.0, color);
            return;
        };
        egui::Image::from_texture(tex).paint_at(ui, rect);
    }

    fn tool_avatar_tab(&self, ui: &mut egui::Ui, name: &str, selected: bool, accent: Color32, c: &ThemeColors) -> bool {
        let bg = if selected { accent.gamma_multiply(0.3) } else { c.surface3 };
        let resp = ui.add(
            egui::Button::new(egui::RichText::new(format!("      {}", name)).color(if selected { accent } else { c.text_secondary })
                .font(FontId::new(13.0, FontFamily::Proportional)))
                .fill(bg).corner_radius(CornerRadius::same(8)).frame(true)
        );
        let center = resp.rect.left_center() + Vec2::new(10.0, 0.0);
        let icon_size = 16.0;
        let icon_rect = egui::Rect::from_center_size(center, Vec2::splat(icon_size));
        if let Some(tex) = self.tool_textures.get(name) {
            egui::Image::from_texture(tex).paint_at(ui, icon_rect);
        } else {
            let color = cli_color(name, 255);
            ui.painter().circle_filled(center, icon_size / 2.0, color);
        }
        resp.clicked()
    }

    fn ordered_tools(&self) -> Vec<String> {
        if self.tool_order.is_empty() {
            return self.detected_clis.clone();
        }
        let mut ordered: Vec<String> = self.tool_order.iter()
            .filter(|t| self.detected_clis.contains(t))
            .cloned()
            .collect();
        for t in &self.detected_clis {
            if !ordered.contains(t) {
                ordered.push(t.clone());
            }
        }
        ordered
    }

    fn rebuild_aggregates(&mut self) {
        self.cli_summaries = aggregate_by_cli(&self.all_records);
        self.model_breakdowns = aggregate_by_model(&self.all_records);
        self.hourly_data = aggregate_hourly(&self.all_records);
        self.summary_total_tokens = self.all_records.iter().map(|r| r.total_tokens).sum();
        self.summary_total_cache = self.all_records.iter().map(|r| r.cache_read_tokens).sum();
        self.summary_total_requests = self.all_records.iter().map(|r| r.request_count).sum();
        self.rebuild_models_cache();
    }

    fn rebuild_models_cache(&mut self) {
        self.models_cache.clear();
        for (name, records) in &self.records_by_cli {
            let models: BTreeSet<&str> = records.iter().map(|r| r.model_name.as_str()).collect();
            self.models_cache.insert(name.clone(), models.iter().map(|s| *s).collect::<Vec<_>>().join(", "));
        }
    }

    fn start_load(&mut self, range: DateRange) {
        self.loading = true;
        self.load_started = true;
        self.all_records.clear();
        self.records_by_cli.clear();
        self.progress_updates.clear();
        self.load_errors.clear();
        self.load_start_time = Some(Instant::now());

        let (progress_tx, progress_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        self.progress_rx = progress_rx;
        self.result_rx = result_rx;

        let readers = detect_readers();
        let detected = readers.iter().map(|r| r.name().to_string()).collect::<Vec<_>>();
        if detected.is_empty() {
            self.loading = false;
            return;
        }
        self.detected_clis = detected;

        for reader in readers {
            let pt = progress_tx.clone();
            let rt = result_tx.clone();
            let rng = range.clone();
            thread::spawn(move || {
                let records = reader.read(&rng, &pt);
                let _ = rt.send(ReaderResult {
                    cli_name: reader.name().to_string(),
                    records,
                    error: None,
                });
            });
        }
    }

    fn poll_channels(&mut self) {
        while let Ok(update) = self.progress_rx.try_recv() {
            if update.done && update.error.is_some() {
                self.load_errors.push(format!("{}: {}", update.cli_name,
                    update.error.as_ref().unwrap()));
            }
            self.progress_updates.insert(update.cli_name.clone(), update);
        }

        while let Ok(result) = self.result_rx.try_recv() {
            if let Some(err) = &result.error {
                self.load_errors.push(format!("{}: {}", result.cli_name, err));
            }
            self.records_by_cli.insert(result.cli_name.clone(), result.records.clone());
            self.all_records.extend(result.records);
            self.progress_updates.entry(result.cli_name.clone()).and_modify(|p| {
                p.done = true;
                p.percent = 1.0;
                let cnt = self.records_by_cli.get(&result.cli_name).map(|v| v.len()).unwrap_or(0);
                p.message = format!("Loaded {} records", cnt);
            });
        }

        while let Ok(msg) = self.update_rx.try_recv() {
            if msg.starts_with("found:") {
                let parts: Vec<&str> = msg.splitn(4, ':').collect();
                if parts.len() == 4 {
                    self.update_info = Some(UpdateInfo {
                        version: parts[1].to_string(),
                        download_url: parts[2].to_string(),
                        asset_name: parts[3].to_string(),
                    });
                    self.update_status = format!("发现新版本 {}", parts[1]);
                }
            } else if msg == "uptodate" {
                self.update_status = "已是最新版本".into();
            } else if msg.starts_with("error:") {
                self.update_status = msg.trim_start_matches("error:").to_string();
            } else if msg.starts_with("dl_ok:") {
                let path = msg.trim_start_matches("dl_ok:");
                self.update_downloaded = Some(std::path::PathBuf::from(path));
                self.update_status = "下载完成".into();
            } else if msg.starts_with("dl_err:") {
                self.update_status = msg.trim_start_matches("dl_err:").to_string();
            }
        }

        if self.loading && !self.detected_clis.is_empty() {
            let all_done = self.detected_clis.iter().all(|name| {
                self.progress_updates.get(name).map(|p| p.done).unwrap_or(false)
            });
            if all_done {
                self.loading = false;
                self.rebuild_aggregates();
            }
        }

        if self.loading && self.last_refresh.elapsed().as_secs_f32() > 1.0 {
            self.rebuild_aggregates();
            self.last_refresh = Instant::now();
        }
    }

    // ── Date Picker ────────────────────────────────────────────

    fn render_date_picker(&mut self, ctx: &egui::Context) {
        if !self.date_picker.open { return; }

        let c = self.tc();
        let target = self.date_picker.target.clone();

        egui::Area::new(egui::Id::new("date_picker_area"))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                Frame {
                    fill: c.surface2,
                    corner_radius: CornerRadius::same(12),
                    stroke: egui::Stroke::new(1.0, c.surface3),
                    shadow: egui::epaint::Shadow { offset: [0, 4], blur: 20, color: Color32::BLACK.gamma_multiply(0.3), ..Default::default() },
                    inner_margin: Margin::symmetric(16, 16),
                    ..Default::default()
                }.show(ui, |ui| {
                    ui.set_min_width(260.0);

                    ui.horizontal(|ui| {
                        if ui.button("\u{25C0}").clicked() {
                            if self.date_picker.month == 1 { self.date_picker.month = 12; self.date_picker.year -= 1; }
                            else { self.date_picker.month -= 1; }
                        }
                        let month_str = match self.date_picker.month {
                            1 => "January", 2 => "February", 3 => "March", 4 => "April",
                            5 => "May", 6 => "June", 7 => "July", 8 => "August",
                            9 => "September", 10 => "October", 11 => "November", 12 => "December",
                            _ => "",
                        };
                        ui.label(egui::RichText::new(format!("{} {}", month_str, self.date_picker.year))
                            .font(FontId::new(14.0, FontFamily::Proportional)).color(c.text_primary).strong());
                        if ui.button("\u{25B6}").clicked() {
                            if self.date_picker.month == 12 { self.date_picker.month = 1; self.date_picker.year += 1; }
                            else { self.date_picker.month += 1; }
                        }
                    });

                    ui.add_space(4.0);
                    let day_names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
                    ui.horizontal(|ui| {
                        for d in &day_names {
                            ui.label(egui::RichText::new(*d).font(FontId::new(11.0, FontFamily::Proportional)).color(c.text_secondary));
                            ui.add_space(8.0);
                        }
                    });

                    let days_in_month = if self.date_picker.month == 12 {
                        NaiveDate::from_ymd_opt(self.date_picker.year + 1, 1, 1)
                    } else {
                        NaiveDate::from_ymd_opt(self.date_picker.year, self.date_picker.month + 1, 1)
                    }.map(|d| d.pred_opt().unwrap().day()).unwrap_or(31) as i32;

                    let first_weekday = NaiveDate::from_ymd_opt(self.date_picker.year, self.date_picker.month, 1)
                        .map(|d| d.weekday().num_days_from_monday()).unwrap_or(0);

                    let now = Local::now().date_naive();
                    let mut day = 1;
                    let mut row = 0;
                    while day <= days_in_month {
                        ui.horizontal(|ui| {
                            for col in 0..7 {
                                if (row == 0 && col < first_weekday as i32) || day > days_in_month {
                                    ui.allocate_ui_with_layout(Vec2::new(24.0, 24.0),
                                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                                        |ui| { ui.label("  "); });
                                } else {
                                    let date = NaiveDate::from_ymd_opt(self.date_picker.year, self.date_picker.month, day as u32).unwrap();
                                    let is_today = date == now;
                                    let selected = if &target == "start" { date == self.custom_start } else { date == self.custom_end };
                                    let btn = egui::Button::new(
                                        egui::RichText::new(day.to_string())
                                            .font(FontId::new(12.0, FontFamily::Proportional))
                                            .color(if is_today { c.accent } else if selected { c.surface } else { c.text_primary })
                                            .strong()
                                    ).min_size(Vec2::new(28.0, 24.0))
                                        .fill(if selected { c.accent } else { egui::Color32::TRANSPARENT })
                                        .corner_radius(CornerRadius::same(4))
                                        .frame(true);
                                    if ui.add(btn).clicked() {
                                        if target == "start" { self.custom_start = date; }
                                        else { self.custom_end = date; }
                                        self.date_picker.open = false;
                                    }
                                    day += 1;
                                }
                            }
                        });
                        row += 1;
                    }

                    ui.add_space(8.0);
                    if ui.button(egui::RichText::new("Close").color(c.text_secondary)).clicked() {
                        self.date_picker.open = false;
                    }
                });
            });
    }

    // ── Settings Window ────────────────────────────────────────

    fn render_settings(&mut self, ctx: &egui::Context) {
        if !self.show_settings { return; }

        let c = self.tc();

        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(Frame {
                fill: c.surface2,
                corner_radius: CornerRadius::same(12),
                inner_margin: Margin::symmetric(20, 20),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.set_min_width(420.0);

                // ── Theme ──
                ui.label(egui::RichText::new(format!("{}  {}", "\u{1F3A8}", self.tr("theme")))
                    .font(FontId::new(15.0, FontFamily::Proportional)).color(c.text_primary).strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (theme, key) in [(Theme::Dark, "theme_dark"), (Theme::Light, "theme_light")] {
                        let sel = self.theme == theme;
                        let bg = if sel { c.accent.gamma_multiply(0.3) } else { c.surface3 };
                        if ui.add(
                            egui::Button::new(egui::RichText::new(self.tr(key))
                                .color(if sel { c.accent } else { c.text_primary }))
                                .fill(bg).corner_radius(CornerRadius::same(8)).frame(true)
                        ).clicked() {
                            self.theme = theme;
                            self.save_config();
                        }
                    }
                });
                ui.add_space(16.0);

                // ── Language ──
                ui.label(egui::RichText::new(format!("{}  {}", "\u{1F310}", self.tr("language")))
                    .font(FontId::new(15.0, FontFamily::Proportional)).color(c.text_primary).strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for lang in [Language::Chinese, Language::English] {
                        let sel = self.language == lang;
                        let bg = if sel { c.accent.gamma_multiply(0.3) } else { c.surface3 };
                        if ui.add(
                            egui::Button::new(egui::RichText::new(lang.label())
                                .color(if sel { c.accent } else { c.text_primary }))
                                .fill(bg).corner_radius(CornerRadius::same(8)).frame(true)
                        ).clicked() {
                            self.language = lang;
                            self.save_config();
                        }
                    }
                });
                ui.add_space(16.0);

                // ── Tool Order ──
                ui.label(egui::RichText::new(format!("{}  {}", "\u{1F504}", self.tr("tool_order_label")))
                    .font(FontId::new(15.0, FontFamily::Proportional)).color(c.text_primary).strong());
                ui.add_space(4.0);
                ui.label(egui::RichText::new("(Drag via up/down buttons)")
                    .font(FontId::new(11.0, FontFamily::Proportional)).color(c.text_secondary));

                let ordered = self.ordered_tools();
                let mut changed = false;
                for i in 0..ordered.len() {
                    let name = &ordered[i];
                    ui.horizontal(|ui| {
                        self.tool_avatar(ui, name, 20.0);
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(name.clone()).color(cli_color(name, 255)).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if i < ordered.len() - 1 {
                                if ui.add(egui::Button::new("\u{25BC}").fill(c.surface3).corner_radius(CornerRadius::same(4)).frame(true)).clicked() {
                                    self.tool_order = ordered.clone();
                                    self.tool_order.swap(i, i + 1);
                                    changed = true;
                                }
                            }
                            if i > 0 {
                                if ui.add(egui::Button::new("\u{25B2}").fill(c.surface3).corner_radius(CornerRadius::same(4)).frame(true)).clicked() {
                                    self.tool_order = ordered.clone();
                                    self.tool_order.swap(i, i - 1);
                                    changed = true;
                                }
                            }
                        });
                    });
                    ui.add_space(2.0);
                }
                if changed { self.save_config(); }
                ui.add_space(16.0);

                // ── Tool Paths ──
                ui.label(egui::RichText::new(format!("{}  {}", "\u{1F4C2}", self.tr("tool_paths")))
                    .font(FontId::new(15.0, FontFamily::Proportional)).color(c.text_primary).strong());
                ui.add_space(4.0);

                let paths = tool_paths();
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for (name, default_path) in &paths {
                        let path = self.tool_path_overrides.get(*name).cloned().unwrap_or_else(|| default_path.clone());
                        ui.horizontal(|ui| {
                            self.tool_avatar(ui, name, 16.0);
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(*name).color(cli_color(name, 255)).strong());
                            if self.detected_clis.contains(&name.to_string()) {
                                ui.label(egui::RichText::new("detected").font(FontId::new(11.0, FontFamily::Proportional)).color(c.green));
                            }
                        });
                        let mut path_edit = path.clone();
                        ui.add_sized(Vec2::new(ui.available_width(), 22.0),
                            egui::TextEdit::singleline(&mut path_edit)
                                .font(FontId::new(11.0, FontFamily::Monospace))
                                .desired_width(f32::INFINITY).text_color(c.text_secondary)
                        );
                        if path_edit != path {
                            self.tool_path_overrides.insert(name.to_string(), path_edit);
                            self.save_config();
                        }
                        ui.add_space(4.0);
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}: {}", self.tr("version"), env!("CARGO_PKG_VERSION")))
                        .font(FontId::new(12.0, FontFamily::Proportional)).color(c.text_secondary));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(egui::RichText::new("Close").color(c.text_primary))
                                .fill(c.surface3).corner_radius(CornerRadius::same(8))
                        ).clicked() {
                            self.show_settings = false;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("更新")
                    .font(FontId::new(15.0, FontFamily::Proportional)).color(c.text_primary).strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("检查更新").color(c.text_primary))
                        .fill(c.surface3).corner_radius(CornerRadius::same(6))
                    ).clicked() {
                        self.update_status = "正在检查...".into();
                        self.update_info = None;
                        self.update_downloaded = None;
                        self.update_rx = spawn_update(UpdateAction::Check, String::new(), String::new());
                    }

                    if !self.update_status.is_empty() {
                        ui.label(egui::RichText::new(&self.update_status)
                            .font(FontId::new(12.0, FontFamily::Proportional)).color(c.text_secondary));
                    }

                    if let Some(info) = &self.update_info {
                        if self.update_downloaded.is_none() {
                            if ui.add(egui::Button::new(egui::RichText::new("下载更新").color(c.text_primary))
                                .fill(c.accent.gamma_multiply(0.3)).corner_radius(CornerRadius::same(6))
                            ).clicked() {
                                self.update_status = "正在下载...".into();
                                let url = info.download_url.clone();
                                let name = info.asset_name.clone();
                                self.update_rx = spawn_update(UpdateAction::Download, url, name);
                            }
                        } else {
                            if ui.add(egui::Button::new(egui::RichText::new("立即更新").color(c.text_primary))
                                .fill(c.green).corner_radius(CornerRadius::same(6))
                            ).clicked() {
                        let downloaded = self.update_downloaded.clone();
                        if let Some(path) = downloaded {
                            match apply_update(&path) {
                                        Ok(()) => { self.update_status = "更新完成，请重启应用".into(); }
                                        Err(e) => { self.update_status = format!("更新失败: {}", e); }
                                    }
                                }
                            }
                        }
                    }
                });
            });
    }
}

// ─── eframe::App ─────────────────────────────────────────────────

impl eframe::App for AiUsageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_channels();
        ctx.request_repaint();

        self.init_textures(ctx);

        let c = self.tc();

        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = self.theme == Theme::Dark;
        style.visuals.widgets.noninteractive.bg_fill = c.surface2;
        style.visuals.widgets.inactive.bg_fill = c.surface3;
        style.visuals.widgets.active.bg_fill = c.accent;
        style.visuals.widgets.hovered.bg_fill = c.surface3;
        style.visuals.window_fill = c.surface;
        style.visuals.panel_fill = c.surface;
        style.visuals.faint_bg_color = c.surface2;
        style.visuals.extreme_bg_color = c.surface;
        style.visuals.override_text_color = Some(c.text_primary);
        style.spacing.item_spacing = Vec2::new(12.0, 8.0);
        style.spacing.window_margin = Margin::same(8);
        ctx.set_style(style);

        self.render_date_picker(ctx);
        self.render_settings(ctx);

        // ── Header ──
        egui::TopBottomPanel::top("header").frame(Frame {
            fill: c.surface2, inner_margin: Margin::symmetric(16, 12), ..Default::default()
        }).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.tr("title"))
                    .font(FontId::new(20.0, FontFamily::Proportional)).color(c.text_primary));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new("\u{2699}").fill(egui::Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(8)).frame(false)).clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    ui.add_space(4.0);
                    if ui.add(egui::Button::new(egui::RichText::new(self.tr("refresh")).color(c.text_primary))
                        .fill(c.surface3).corner_radius(CornerRadius::same(6))).clicked() {
                        self.start_load(self.date_range.clone());
                    }
                    ui.add_space(8.0);
                    self.time_range_selector(ui, c);
                });
            });
        });

        // ── Tabs ──
        egui::TopBottomPanel::top("tabs").frame(Frame {
            fill: c.surface, inner_margin: Margin::symmetric(16, 4), ..Default::default()
        }).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let sel = self.current_tab == Tab::Summary;
                if tab_button(ui, self.tr("summary_tab"), sel, c.accent, c) {
                    self.current_tab = Tab::Summary;
                }
                let ordered = self.ordered_tools();
                for name in &ordered {
                    let count = self.records_by_cli.get(name).map(|r| r.len()).unwrap_or(0);
                    let selected = matches!(&self.current_tab, Tab::Tool(n) if n == name);
                    let clr = cli_color(name, 255);
                    if self.tool_avatar_tab(ui, name, selected, clr, c) {
                        self.current_tab = Tab::Tool(name.clone());
                    }
                    if count > 0 {
                        let total = self.records_by_cli.get(name)
                            .map(|r| r.iter().map(|r| r.total_tokens).sum()).unwrap_or(0);
                        ui.label(egui::RichText::new(format!("({})", format_tokens(total)))
                            .font(FontId::new(11.0, FontFamily::Proportional)).color(c.text_secondary));
                    }
                }
            });
        });

        // ── Content ──
        egui::CentralPanel::default().frame(Frame {
            fill: c.surface, inner_margin: Margin::symmetric(24, 16), ..Default::default()
        }).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.loading { self.render_loading(ui, c); }
                match self.current_tab.clone() {
                    Tab::Summary => self.render_summary(ui, c),
                    Tab::Tool(ref name) => self.render_tool_tab(ui, name, c),
                }
                ui.add_space(40.0);
            });
        });
    }
}

// ─── Render Methods ──────────────────────────────────────────────

impl AiUsageApp {
    fn render_loading(&mut self, ui: &mut egui::Ui, c: &ThemeColors) {
        ui.add_space(8.0);
        let elapsed = self.load_start_time.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);

        Frame {
            fill: c.surface2, corner_radius: CornerRadius::same(12),
            inner_margin: Margin::symmetric(20, 16), ..Default::default()
        }.show(ui, |ui| {
            ui.heading(egui::RichText::new(self.tr("loading_title")).color(c.text_primary));
            ui.add_space(12.0);

            let total = self.detected_clis.len();
            let done = self.detected_clis.iter()
                .filter(|n| self.progress_updates.get(*n).map(|p| p.done).unwrap_or(false)).count();
            let overall = if total > 0 { done as f32 / total as f32 } else { 0.0 };

            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
            ui.painter().rect_filled(rect, CornerRadius::same(12), c.surface3);
            if overall > 0.0 {
                let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * overall, rect.height()));
                let clr = if overall >= 1.0 { c.green } else { c.accent };
                ui.painter().rect_filled(fill_rect, CornerRadius::same(12), clr);
            }
            ui.painter().text(rect.center(), Align2::CENTER_CENTER,
                &format!("{} / {} ({:.1}s)", done, total, elapsed),
                FontId::new(13.0, FontFamily::Proportional), c.text_primary);

            ui.add_space(12.0);

            for name in &self.detected_clis {
                let p = self.progress_updates.get(name);
                let percent = p.map(|p| p.percent).unwrap_or(0.0);
                let msg = p.map(|p| p.message.clone()).unwrap_or_else(|| "Waiting...".into());
                let d = p.map(|p| p.done).unwrap_or(false);
                let err = p.and_then(|p| p.error.clone());

                ui.horizontal(|ui| {
                    let icon = if d && err.is_none() { "\u{2705}" } else if err.is_some() { "\u{274C}" } else { "\u{23F3}" };
                    ui.label(egui::RichText::new(format!("{} {}", icon, name))
                        .font(FontId::new(13.0, FontFamily::Proportional)).color(cli_color(name, 255)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(msg).font(FontId::new(12.0, FontFamily::Proportional)).color(c.text_secondary));
                    });
                });

                let bar_rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 6.0));
                let bar_rect = egui::Rect::from_min_size(bar_rect.min + Vec2::new(0.0, 2.0), Vec2::new(bar_rect.width(), 6.0));
                ui.painter().rect_filled(bar_rect, CornerRadius::same(3), c.surface3);
                if percent > 0.0 {
                    let fill = egui::Rect::from_min_size(bar_rect.min, Vec2::new(bar_rect.width() * percent.min(1.0), bar_rect.height()));
                    ui.painter().rect_filled(fill, CornerRadius::same(3), cli_color(name, 200));
                }
                ui.add_space(6.0);
            }
        });
        ui.add_space(8.0);
    }

    fn render_summary(&mut self, ui: &mut egui::Ui, c: &ThemeColors) {
        if self.all_records.is_empty() && !self.loading {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(egui::RichText::new(self.tr("no_data_today"))
                    .font(FontId::new(18.0, FontFamily::Proportional)).color(c.text_secondary));
                ui.label(egui::RichText::new(self.tr("try_change_range"))
                    .font(FontId::new(14.0, FontFamily::Proportional)).color(c.text_secondary));
            });
            return;
        }
        if self.all_records.is_empty() { return; }

        ui.add_space(4.0);

        // Summary Statistics
        egui::CollapsingHeader::new(egui::RichText::new(format!("{}  {}", "\u{1F4CA}", self.tr("summary_stats")))
                .font(FontId::new(18.0, FontFamily::Proportional)).color(c.text_primary).strong())
            .default_open(true).show(ui, |ui| {
                ui.add_space(8.0);
                Frame { fill: c.surface2, corner_radius: CornerRadius::same(12), inner_margin: Margin::symmetric(16, 16), ..Default::default() }.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        stat_card(ui, self.tr("total_tokens"), &format_tokens_full(self.summary_total_tokens), c.accent_light, "", c);
                        stat_card(ui, self.tr("cache_hit_rate"),
                            &format!("{:.1}%", if self.summary_total_tokens > 0 { self.summary_total_cache as f64 / self.summary_total_tokens as f64 * 100.0 } else { 0.0 }),
                            c.green, &format!("{} {}", self.tr("cached"), format_tokens_full(self.summary_total_cache)), c);
                        stat_card(ui, self.tr("total_requests"), &format_tokens_full(self.summary_total_requests), c.cyan, "", c);
                        stat_card(ui, self.tr("active_tools"), &self.detected_clis.len().to_string(), c.yellow, "", c);
                    });
                });
            });

        ui.add_space(8.0);

        // Tool Breakdown
        egui::CollapsingHeader::new(egui::RichText::new(format!("{}  {}", "\u{1F527}", self.tr("tool_breakdown")))
                .font(FontId::new(18.0, FontFamily::Proportional)).color(c.text_primary).strong())
            .default_open(true).show(ui, |ui| {
                ui.add_space(8.0);
                for name in self.ordered_tools() {
                    let records = match self.records_by_cli.get(&name) {
                        Some(r) if !r.is_empty() => r,
                        _ => continue,
                    };
                    let total = records.iter().map(|r| r.total_tokens).sum::<u64>();
                    let cnt = records.len();
                    let clr = cli_color(&name, 255);

                    Frame { fill: c.surface2, corner_radius: CornerRadius::same(10), inner_margin: Margin::symmetric(16, 12), ..Default::default() }.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.tool_avatar(ui, &name, 22.0);
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(&name).font(FontId::new(16.0, FontFamily::Proportional)).color(clr).strong());
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new(format_tokens_full(total))
                                .font(FontId::new(16.0, FontFamily::Proportional)).color(c.text_primary).strong());
                            ui.label(egui::RichText::new(format!("{} {}", cnt, self.tr("sessions")))
                                .font(FontId::new(12.0, FontFamily::Proportional)).color(c.text_secondary));
                        });
                        let tool_models = aggregate_by_model(records);
                        if !tool_models.is_empty() {
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                for mb in &tool_models {
                                    let rate = if mb.total_tokens > 0 { mb.cache_read as f64 / mb.total_tokens as f64 * 100.0 } else { 0.0 };
                                    let lbl = format!("{} ({} / {:.0}%)", mb.model_name, format_tokens(mb.total_tokens), rate);
                                    ui.label(egui::RichText::new(lbl)
                                        .font(FontId::new(11.0, FontFamily::Proportional))
                                        .color(if rate > 50.0 { c.green } else { c.text_secondary }));
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                }
            });

        ui.add_space(8.0);

        // Model Usage Breakdown
        if !self.model_breakdowns.is_empty() {
            egui::CollapsingHeader::new(egui::RichText::new(format!("{}  {}", "\u{2699}", self.tr("model_breakdown")))
                    .font(FontId::new(18.0, FontFamily::Proportional)).color(c.text_primary).strong())
                .default_open(true).show(ui, |ui| {
                    ui.add_space(8.0);
                    Frame { fill: c.surface2, corner_radius: CornerRadius::same(12), inner_margin: Margin::symmetric(20, 16), ..Default::default() }.show(ui, |ui| {
                        let headers = [self.tr("tool"), self.tr("model"), self.tr("requests"),
                            self.tr("tokens"), self.tr("cache"), self.tr("cache_pct"), self.tr("cost")];

                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            egui::Grid::new("model_grid").striped(true).show(ui, |ui| {
                                for h in &headers {
                                    ui.label(egui::RichText::new(*h).font(FontId::new(12.0, FontFamily::Proportional)).color(c.accent_light).strong());
                                }
                                ui.end_row();

                                for mb in &self.model_breakdowns {
                                    let cache_pct = if mb.total_tokens > 0 { mb.cache_read as f64 / mb.total_tokens as f64 * 100.0 } else { 0.0 };
                                    let cost_str = if mb.cost_cents > 0 { format!("${:.2}", mb.cost_cents as f64 / 100.0) } else { "-".into() };

                                    ui.horizontal(|ui| {
                                        self.tool_avatar(ui, &mb.cli_name, 14.0);
                                        ui.add_space(2.0);
                                        ui.label(egui::RichText::new(&mb.cli_name).color(cli_color(&mb.cli_name, 255)));
                                    });
                                    ui.label(egui::RichText::new(&mb.model_name).color(c.text_primary));
                                    ui.label(egui::RichText::new(&mb.request_count.to_string()).color(c.text_secondary));
                                    ui.label(egui::RichText::new(&format_tokens_full(mb.total_tokens)).color(c.text_primary).strong());
                                    ui.label(egui::RichText::new(&format_tokens_full(mb.cache_read))
                                        .color(if cache_pct > 50.0 { c.green } else { c.text_secondary }));
                                    ui.label(egui::RichText::new(&format!("{:.0}%", cache_pct))
                                        .color(if cache_pct > 50.0 { c.green } else { c.orange }));
                                    ui.label(egui::RichText::new(&cost_str).color(c.text_secondary));
                                    ui.end_row();
                                }
                            });
                        });
                    });
                });
        }

        ui.add_space(8.0);

        // Hourly Trend Chart
        if !self.hourly_data.is_empty() {
            egui::CollapsingHeader::new(egui::RichText::new(format!("{}  {}", "\u{1F4C8}", self.tr("hourly_chart")))
                    .font(FontId::new(18.0, FontFamily::Proportional)).color(c.text_primary).strong())
                .default_open(true).show(ui, |ui| {
                    ui.add_space(8.0);
                    Frame { fill: c.surface2, corner_radius: CornerRadius::same(12), inner_margin: Margin::symmetric(20, 16), ..Default::default() }.show(ui, |ui| {
                        let points: Vec<[f64; 2]> = self.hourly_data.iter().map(|p| [p.hour as f64, p.total_tokens as f64]).collect();
                        let cache_points: Vec<[f64; 2]> = self.hourly_data.iter().map(|p| [p.hour as f64, p.cache_hit as f64]).collect();

                        let line_total = Line::new(PlotPoints::from(points)).color(c.accent_light).width(2.5).name("Total Tokens");
                        let line_cache = Line::new(PlotPoints::from(cache_points)).color(c.green).width(2.0).name("Cache Hit").highlight(true);

                        Plot::new("hourly_chart").height(200.0).show_background(false).show_axes(true).show_grid(true)
                            .x_axis_label("Hour").y_axis_label("Tokens").show(ui, |plot_ui| {
                                plot_ui.line(line_total); plot_ui.line(line_cache);
                            });
                    });
                });
        }
    }

    fn render_tool_tab(&mut self, ui: &mut egui::Ui, cli_name: &str, c: &ThemeColors) {
        let records = match self.records_by_cli.get(cli_name) {
            Some(r) => r,
            None => {
                ui.label(egui::RichText::new(if self.loading { self.tr("loading") } else { self.tr("no_data") }).color(c.text_secondary));
                return;
            }
        };

        if records.is_empty() {
            ui.label(egui::RichText::new(self.tr("no_period_data")).color(c.text_secondary));
            return;
        }

        let total_tokens: u64 = records.iter().map(|r| r.total_tokens).sum();
        let total_cache: u64 = records.iter().map(|r| r.cache_read_tokens).sum();
        let total_req: u64 = records.iter().map(|r| r.request_count).sum();
        let models: BTreeSet<&str> = records.iter().map(|r| r.model_name.as_str()).collect();
        let models_str = self.models_cache.get(cli_name).cloned().unwrap_or_default();
        let clr = cli_color(cli_name, 255);

        // Header
        Frame { fill: c.surface2, corner_radius: CornerRadius::same(12), inner_margin: Margin::symmetric(20, 20), ..Default::default() }.show(ui, |ui| {
            ui.horizontal(|ui| {
                self.tool_avatar(ui, cli_name, 28.0);
                ui.add_space(8.0);
                ui.label(egui::RichText::new(cli_name).font(FontId::new(20.0, FontFamily::Proportional)).color(clr));
                ui.label(egui::RichText::new(format!("{} {} / {} {}", models.len(), self.tr("model"),
                    records.len(), self.tr("sessions")))
                    .font(FontId::new(13.0, FontFamily::Proportional)).color(c.text_secondary));
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                stat_card(ui, self.tr("total_tokens"), &format_tokens_full(total_tokens), clr, "", c);
                stat_card(ui, self.tr("cache_hit_rate"),
                    &format!("{:.1}%", if total_tokens > 0 { total_cache as f64 / total_tokens as f64 * 100.0 } else { 0.0 }),
                    c.green, &format!("{} {}", self.tr("cached"), format_tokens_full(total_cache)), c);
                stat_card(ui, self.tr("total_requests"), &total_req.to_string(), c.cyan, "", c);
                stat_card(ui, self.tr("model"), &models.len().to_string(), c.yellow, &models_str, c);
            });
        });
        ui.add_space(16.0);

        // Model breakdown
        let tool_models = aggregate_by_model(records);
        Frame { fill: c.surface2, corner_radius: CornerRadius::same(12), inner_margin: Margin::symmetric(20, 16), ..Default::default() }.show(ui, |ui| {
            ui.label(egui::RichText::new(format!("{}  {}", "\u{1F916}", self.tr("model_breakdown")))
                .font(FontId::new(16.0, FontFamily::Proportional)).color(c.text_primary));
            ui.add_space(8.0);

            let headers = [self.tr("model"), self.tr("requests"), self.tr("tokens"),
                self.tr("input"), self.tr("output"), self.tr("cache"), self.tr("cache_pct")];

            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("tool_model_grid").striped(true).show(ui, |ui| {
                    for h in &headers {
                        ui.label(egui::RichText::new(*h).font(FontId::new(12.0, FontFamily::Proportional)).color(clr).strong());
                    }
                    ui.end_row();

                    for mb in &tool_models {
                        let cache_pct = if mb.total_tokens > 0 { mb.cache_read as f64 / mb.total_tokens as f64 * 100.0 } else { 0.0 };
                        let cache_color = if cache_pct > 50.0 { c.green } else if cache_pct > 20.0 { c.yellow } else { c.orange };

                        ui.label(egui::RichText::new(&mb.model_name).color(c.text_primary));
                        ui.label(egui::RichText::new(&mb.request_count.to_string()).color(c.text_secondary));
                        ui.label(egui::RichText::new(&format_tokens_full(mb.total_tokens)).color(c.text_primary).strong());
                        ui.label(egui::RichText::new(&format_tokens_full(mb.prompt_tokens)).color(c.text_secondary));
                        ui.label(egui::RichText::new(&format_tokens_full(mb.completion_tokens)).color(c.text_secondary));
                        ui.label(egui::RichText::new(&format_tokens_full(mb.cache_read)).color(cache_color));
                        ui.label(egui::RichText::new(&format!("{:.0}%", cache_pct)).color(cache_color));
                        ui.end_row();
                    }
                });
            });
        });
    }
}

// ─── Helper Functions ────────────────────────────────────────────

fn tab_button(ui: &mut egui::Ui, label: &str, selected: bool, accent: Color32, c: &ThemeColors) -> bool {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(if selected { accent } else { c.text_secondary })
            .font(FontId::new(13.0, FontFamily::Proportional)))
            .fill(if selected { accent.gamma_multiply(0.3) } else { c.surface3 })
            .corner_radius(CornerRadius::same(8)).frame(true)
    ).clicked()
}

fn stat_card(ui: &mut egui::Ui, title: &str, value: &str, color: Color32, subtitle: &str, c: &ThemeColors) {
    let available = (ui.available_width() - 24.0) / 4.0;
    Frame { fill: c.surface3, corner_radius: CornerRadius::same(10), inner_margin: Margin::symmetric(14, 12), ..Default::default() }.show(ui, |ui| {
        ui.set_min_width(available.max(120.0));
        ui.set_max_width(available.max(120.0));
        ui.label(egui::RichText::new(title).font(FontId::new(11.0, FontFamily::Proportional)).color(c.text_secondary));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(value).font(FontId::new(22.0, FontFamily::Proportional)).color(color).strong());
        if !subtitle.is_empty() {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(subtitle).font(FontId::new(11.0, FontFamily::Proportional)).color(c.text_secondary));
        }
    });
    ui.add_space(8.0);
}

impl AiUsageApp {
    fn time_range_selector(&mut self, ui: &mut egui::Ui, c: &ThemeColors) {
        let current_type = self.date_range.range_type;
        let types: &[(DateRangeType, &str)] = &[
            (DateRangeType::Today, "today"),
            (DateRangeType::Week, "week"),
            (DateRangeType::Month, "month"),
            (DateRangeType::Custom, "custom"),
        ];

        ui.horizontal(|ui| {
            for (rt, key) in types {
                let sel = current_type == *rt;
                let bg = if sel { c.accent.gamma_multiply(0.3) } else { c.surface3 };
                let color = if sel { c.accent_light } else { c.text_secondary };

                if ui.add(egui::Button::new(egui::RichText::new(self.tr(key)).color(color)
                    .font(FontId::new(12.0, FontFamily::Proportional)))
                    .fill(bg).corner_radius(CornerRadius::same(6)).frame(true)).clicked() {
                    let new_range = match rt {
                        DateRangeType::Today => DateRange::today(),
                        DateRangeType::Week => DateRange::this_week(),
                        DateRangeType::Month => DateRange::this_month(),
                        DateRangeType::Custom => DateRange { range_type: DateRangeType::Custom, start: self.custom_start, end: self.custom_end },
                    };
                    self.date_range = new_range.clone();
                    self.start_load(new_range);
                }
            }

            if current_type == DateRangeType::Custom {
                ui.add_space(8.0);

                if ui.add(egui::Button::new(egui::RichText::new(self.custom_start.format("%Y-%m-%d").to_string())
                    .font(FontId::new(12.0, FontFamily::Monospace)).color(c.text_primary))
                    .fill(c.surface3).corner_radius(CornerRadius::same(6))).clicked() {
                    self.date_picker.target = "start".into();
                    self.date_picker.year = self.custom_start.year();
                    self.date_picker.month = self.custom_start.month();
                    self.date_picker.open = true;
                }

                ui.label(egui::RichText::new("->").color(c.text_secondary));

                if ui.add(egui::Button::new(egui::RichText::new(self.custom_end.format("%Y-%m-%d").to_string())
                    .font(FontId::new(12.0, FontFamily::Monospace)).color(c.text_primary))
                    .fill(c.surface3).corner_radius(CornerRadius::same(6))).clicked() {
                    self.date_picker.target = "end".into();
                    self.date_picker.year = self.custom_end.year();
                    self.date_picker.month = self.custom_end.month();
                    self.date_picker.open = true;
                }

                ui.add_space(4.0);

                if ui.add(egui::Button::new(egui::RichText::new(self.tr("apply"))
                    .font(FontId::new(12.0, FontFamily::Proportional)).color(c.text_primary))
                    .fill(c.accent.gamma_multiply(0.3))
                    .corner_radius(CornerRadius::same(6))).clicked() {
                    let new_range = DateRange { range_type: DateRangeType::Custom, start: self.custom_start, end: self.custom_end };
                    self.date_range = new_range.clone();
                    self.start_load(new_range);
                }
            }
        });
    }
}

fn setup_fonts(cc: &eframe::CreationContext) {
    let mut fonts = egui::FontDefinitions::default();

    let font_candidates = [
        ("pingfang", "/System/Library/Fonts/PingFang.ttc"),
        ("stheitisc", "/System/Library/Fonts/STHeiti Light.ttc"),
        ("notocjk", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        ("notocjk2", "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
        ("arialuni", "/Library/Fonts/Arial Unicode.ttf"),
        ("msyh", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("simsun", "C:\\Windows\\Fonts\\simsun.ttc"),
        ("emojifont", "/System/Library/Fonts/Apple Color Emoji.ttc"),
        ("notoemoji", "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"),
        ("segoeuiemoji", "C:\\Windows\\Fonts\\seguiemj.ttf"),
    ];

    for (name, path) in &font_candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                name.to_string(),
                std::sync::Arc::new(egui::FontData::from_owned(data.into())),
            );
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
                .push(name.to_string());
        }
    }

    cc.egui_ctx.set_fonts(fonts);
}

fn main() -> Result<(), eframe::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("ai_usage_statistics=info".parse().unwrap()))
        .init();

    let config = load_config();
    let window_title = match config.language.as_str() {
        "English" => "AI Usage Statistics",
        _ => "AI 使用量统计",
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title(window_title),
        ..Default::default()
    };

    eframe::run_native(
        window_title,
        options,
        Box::new(|cc| {
            setup_fonts(cc);
            Ok(Box::new(AiUsageApp::default()))
        }),
    )
}
