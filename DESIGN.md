# AI Usage Statistics - 实时多 Agent 用量统计工具

## 项目概述

跨平台（Windows/Linux/Mac）的 AI CLI 工具 Token 用量统计工具。

**核心思路：直接读取各 CLI 工具已有的本地数据，按需聚合展示。**
- ❌ 不做网络拦截/MITM
- ❌ 不需要后台服务
- ✅ GUI 打开时扫描已安装 CLI → 读取其本地数据 → 聚合展示
- ✅ 关闭后无任何开销，数据由各 CLI 自己维护

## 支持的 CLI 工具数据源

| CLI | 数据路径 | 格式 | 关键字段 | 区分 CLI/GUI |
|-----|---------|------|---------|-------------|
| **opencode** | `~/.local/share/opencode/opencode.db` | SQLite | `tokens_input`, `tokens_output`, `tokens_cache_read`, `tokens_cache_write`, `tokens_reasoning`, `cost`, `model`(JSON), `time_created`, `agent` | `agent` 字段 |
| **qwen** (Qwen Code) | `~/.qwen/projects/*/chats/*.jsonl` | JSONL | `usageMetadata.promptTokenCount`, `candidatesTokenCount`, `totalTokenCount`, `cachedContentTokenCount`, `thoughtsTokenCount`, `model` | 独立 VS Code 扩展路径 |
| **claude** (Claude Code) | `~/.claude/usage-data/session-meta/*.json` + `stats-cache.json` | JSON | `input_tokens`, `output_tokens`, `model`, `start_time` | CLI vs Desktop 不同目录 |
| **hermes** | `~/.hermes/state.db` | SQLite | `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_write_tokens`, `reasoning_tokens`, `model`, `started_at`, `source` | `source` 字段 |
| **codex** (OpenAI Codex) | `~/.codex/state_5.sqlite` + `.codex/sessions/*/*.jsonl` | SQLite + JSONL | `threads.tokens_used`, `threads.model`, `threads.source`, JSONL 中 `input_token_details.cached_tokens` | `source` 字段 |

## Adapter 模式

每个 CLI 实现一个 `UsageReader` trait。读取逻辑在独立线程中异步执行，通过 channel 发送进度和结果。

```rust
/// Adapter trait
trait UsageReader: Send + Sync {
    /// 工具名称
    fn name(&self) -> &'static str;

    /// 是否已安装
    fn is_installed(&self) -> bool;

    /// 读取指定日期范围的用量（内部通过 progress_tx 汇报进度）
    fn read(
        &self,
        range: DateRange,
        progress_tx: ProgressSender,
    ) -> Result<Vec<UsageRecord>>;
}

/// 进度汇报
struct ProgressUpdate {
    cli_name: String,
    stage: ProgressStage,
    percent: f32,       // 0.0 ~ 1.0
    message: String,
}

enum ProgressStage {
    Scanning,     // 扫描数据文件
    Parsing,      // 解析中
    Complete,     // 该工具读取完成
    Error(String),// 出错
}

/// 统一聚合后的记录
struct UsageRecord {
    cli_name: String,
    source_type: SourceType,  // Cli | Gui | Unknown
    model_name: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,   // 缓存命中 token
    cache_write_tokens: u64,  // 缓存写入 token
    reasoning_tokens: u64,
    request_count: u64,
    timestamp: DateTime<Utc>,
}

enum SourceType { Cli, Gui, Unknown }
```

## 加载与进度机制

```
用户打开 GUI（默认当天）
    │
    ├── 启动 N 个并行任务（每个已安装 CLI 一个）
    │
    ├── 任务 A: opencode ─── 开始扫描 ─── 解析中 ─── 完成  → 发结果到 chan
    │   ├── progress: [===         ] 30%  "扫描 SQLite..."
    │   ├── progress: [========    ] 60%  "解析 session..."
    │   └── progress: [============] 100% "完成"
    │
    ├── 任务 B: qwen ─────── 开始扫描 ─── 解析中 ─── 完成  → 发结果到 chan
    ├── 任务 C: claude ───── 开始扫描 ─── 解析中 ─── 完成  → 发结果到 chan
    ├── 任务 D: hermes ───── 开始扫描 ─── 解析中 ─── 完成  → 发结果到 chan
    └── 任务 E: codex ────── 开始扫描 ─── 解析中 ─── 完成  → 发结果到 chan
                                │              │
                                ▼              ▼
                          ┌──────────┐  ┌──────────┐
                          │ 进度条    │  │ 结果缓冲区│
                          │ ProgressBar│  │ HashMap  │
                          │ 实时更新  │  │ 按工具存储│
                          └──────────┘  └─────┬────┘
                                              │
                    ┌─────────────────────────┤
                    │                         │
                    ▼                         ▼
          ┌─────────────────┐       ┌─────────────────┐
          │ 工具选项卡       │       │ 总计选项卡       │
          │ 每完成一个就添加  │       │ 全部完成才展示    │
          │ 立即渲染数据     │       │ 聚合所有工具数据  │
          └─────────────────┘       └─────────────────┘
```

## GUI 布局

```
┌──────────────────────────────────────────────────────────────┐
│  AI Usage Statistics - Token 用量统计           [─][□][×] │
│                                                              │
│  [今天 ●]  [本周]  [本月]  [自定义: 2026-05-01 ~ 2026-05-22] │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  加载进度: ━━━━━━━━━━━━━━━━ 75%                              │
│  ┌─ opencode  ████████████████████ 100%  423K tokens       ─┐│
│  ├─ claude    ████████████████░░░░  80%                      ││
│  ├─ hermes    ████████████████████ 100%  156K tokens       ─┘│
│  ├─ qwen      ██████████░░░░░░░░░░  50%                      │
│  └─ codex     ████░░░░░░░░░░░░░░░░  20%  解析 JSONL...      │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  [总计]  [opencode]  [claude]  [hermes]  [qwen]  [codex]   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  今日总览（等待所有工具加载完成...）                   │   │
│  │                                                      │   │
│  │  总 Token: 1,234,567   总缓存命中: 789,012 (64%)     │   │
│  │                                                      │   │
│  │  各工具占比:                                         │   │
│  │  opencode ████████████████ 45%  556,055              │   │
│  │  claude   ██████████      25%  308,641              │   │
│  │  hermes   ███████         20%  246,913              │   │
│  │  qwen     ████            10%  123,456              │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  工具明细 (opencode)                                   │   │
│  │                                                      │   │
│  │  模型              请求数    Token    缓存命中    占比   │   │
│  │  ────────────────────────────────────────────────    │   │
│  │  deepseek-v4-flash    45   234,567   187,654   80%    │   │
│  │  glm-5                12    98,234     3,456    3%    │   │
│  │  mistral-7b            8    56,789    45,678   80%    │   │
│  │  ...                                                   │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  走势图                                               │   │
│  │  ┌─────────────────────────────────────────────┐    │   │
│  │  │   ╱╲      ╱╲         （每小时 token 量）      │    │   │
│  │  │  ╱  ╲    ╱  ╲    ╱╲                           │    │   │
│  │  │ ╱    ╲  ╱    ╲  ╱  ╲                          │    │   │
│  │  │╱      ╲╱      ╲╱    ╲                         │    │   │
│  │  └─────────────────────────────────────────────┘    │   │
│  │  00:00  04:00  08:00  12:00  16:00  20:00  Now     │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## 关键设计决策

### 1. 异步并发加载

```rust
/// 加载管理器
async fn load_all(
    readers: Vec<Box<dyn UsageReader>>,
    range: DateRange,
) -> mpsc::Receiver<ReaderResult> {
    let (result_tx, result_rx) = mpsc::channel(64);
    let (progress_tx, _) = broadcast::channel(32);

    for reader in readers {
        let tx = result_tx.clone();
        let range = range.clone();
        tokio::spawn(async move {
            let records = reader.read(range, progress_tx);
            tx.send(ReaderResult {
                cli_name: reader.name(),
                records,
            }).await;
        });
    }

    result_rx
}
```

### 2. 增量渲染

```rust
/// GUI 状态
struct AppState {
    // 已完成的 reader 结果（每完成一个立即插入）
    reader_results: HashMap<String, ReaderResult>,
    // 进度状态
    progress: HashMap<String, ProgressState>,
    // 全部是否完成
    all_done: bool,
    // 当前选中的 tab
    selected_tab: Tab,
    // 当前时间范围
    date_range: DateRange,
}

// 每帧检查 channel，有新结果立即更新 UI
fn update(&mut self, ctx: &egui::Context) {
    while let Ok(result) = self.result_rx.try_recv() {
        self.reader_results.insert(result.cli_name.clone(), result);
        // 如果这个工具刚完成，自动切到它的 tab（首次）
        // 工具栏新增一个 tab 按钮
    }
    while let Ok(progress) = self.progress_rx.try_recv() {
        self.progress.insert(progress.cli_name.clone(), progress);
    }
}
```

### 3. 时间范围选择

```rust
enum DateRange {
    Today,
    Week,        // 本周一 ~ 今天
    Month,       // 本月1号 ~ 今天
    Custom(NaiveDate, NaiveDate),
}

impl DateRange {
    fn start_end(&self) -> (NaiveDate, NaiveDate) {
        let today = Local::now().date_naive();
        match self {
            DateRange::Today => (today, today),
            DateRange::Week => (today - Duration::days(today.weekday().num_days_from_monday() as i64), today),
            DateRange::Month => (today.with_day(1).unwrap(), today),
            DateRange::Custom(start, end) => (*start, *end),
        }
    }
}
```

### 4. 缓存读取耗时优化

| 工具 | 主要耗时 | 优化策略 |
|------|---------|---------|
| opencode | SQLite 1 次查询 | ✅ 快 |
| qwen | 扫描大量 JSONL 文件 | 缓存上次扫描位置，增量读取 |
| claude | 读取 ~73 个 JSON 文件 | 并行读取，使用 `tokio::fs` |
| hermes | SQLite 1 次查询 | ✅ 快 |
| codex | SQLite 快 + JSONL 需额外解析 | 先从 SQLite 读 `tokens_used`，后台懒加载 JSONL 中的缓存数据 |

对于 qwen 和 codex 的 JSONL：
```
第一次加载：全量扫描，记录每个文件的偏移量
后续加载：只读新增内容（seeko 到上次位置）
```

## 项目结构

```
ai-usage-statistics/
├── Cargo.toml
├── crates/
│   ├── core/                 # 数据模型 + 聚合引擎 + DateRange
│   │   └── src/
│   │       ├── models.rs     # UsageRecord, ProgressUpdate
│   │       ├── aggregator.rs # 按工具/模型分组汇总
│   │       └── range.rs      # DateRange
│   ├── readers/              # 各 CLI adapter + 注册
│   │   └── src/
│   │       ├── lib.rs        # UsageReader trait + registry
│   │       ├── opencode.rs
│   │       ├── qwen.rs
│   │       ├── claude.rs
│   │       ├── hermes.rs
│   │       └── codex.rs
│   └── gui/                  # egui Dashboard
│       └── src/
│           ├── main.rs
│           ├── app.rs        # AppState, 生命周期
│           ├── dashboard.rs  # 总计 tab
│           ├── cli_tab.rs    # 单个工具 tab
│           ├── progress.rs   # 进度条组件
│           └── timeline.rs   # 时间选择器
└── docs/
    └── ARCHITECTURE.md
```

## 各 Adapter 实现要点

### opencode

```sql
-- SQLite: 一次查询当天所有 session
SELECT tokens_input, tokens_output, tokens_cache_read,
       tokens_cache_write, tokens_reasoning, model, time_created
FROM session
WHERE date(time_created / 1000, 'unixepoch') = ?
ORDER BY time_created DESC;
```

### hermes

```sql
-- SQLite: 一次查询当天所有 session
SELECT input_tokens, output_tokens, cache_read_tokens,
       cache_write_tokens, reasoning_tokens, model, started_at,
       estimated_cost_usd
FROM sessions
WHERE date(started_at, 'unixepoch') = ?
ORDER BY started_at DESC;
```

### codex

```sql
-- SQLite: 查询当天 threads（快速）
SELECT model, tokens_used, source, created_at, rollout_path
FROM threads
WHERE date(created_at, 'unixepoch') = ?;

-- JSONL: 从 rollout_path 的文件中解析缓存数据（后台懒加载）
-- 搜索 input_token_details.cached_tokens
-- 或 usageMetadata.cachedContentTokenCount（取决于 API 格式）
-- 汇总到对应 thread 的 cache_tokens 字段
```

### qwen

```rust
// 遍历 ~/.qwen/projects/*/chats/*.jsonl
// 每行 JSON，筛选 date 范围内的行
// 提取 usageMetadata 或 ui_telemetry 事件
// usageMetadata: promptTokenCount, candidatesTokenCount,
//                totalTokenCount, cachedContentTokenCount
// ui_telemetry:  input_token_count, output_token_count,
//                cached_content_token_count
```

### claude

```rust
// 读取 ~/.claude/usage-data/session-meta/*.json
// 每个文件一个 session
// input_tokens, output_tokens, model, start_time
// 缓存数据在 stats-cache.json 有聚合值
// 或从 JSONL 细粒度解析
```

## 技术要点

1. **跨平台数据路径**：通过 `dirs`/`dirs-next` crate 统一处理各平台数据目录差异
2. **只读访问**：所有 adapter 以只读方式操作，不修改任何数据
3. **增量读取**：记录每个 JSONL 文件的读取偏移，下次只读新增行
4. **优雅降级**：某个工具读取失败不影响其他工具，错误信息单独展示
5. **首次加载缓存**：首次全量扫描后生成索引缓存，后续打开秒出
