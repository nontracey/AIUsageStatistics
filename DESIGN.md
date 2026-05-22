# AI Usage Statistics - 实时多 Agent 用量统计工具

## 项目概述

跨平台（Windows/Linux/Mac）的 AI CLI 工具 Token 用量统计工具。

**核心思路：直接读取各 CLI 工具已有的本地数据，按需聚合展示。**
- ❌ 不做网络拦截/MITM
- ❌ 不需要后台服务
- ✅ GUI 打开时扫描已安装 CLI → 读取其本地数据 → 聚合展示
- ✅ 关闭后无任何开销，数据由各 CLI 自己维护

## 支持的 CLI 工具数据源

| CLI | 数据路径 | 格式 | 会话 ID | 区分 CLI/GUI |
|-----|---------|------|---------|-------------|
| **opencode** | `~/.local/share/opencode/opencode.db` | SQLite | `session.name` UUID | `agent` 字段 |
| **hermes** | `~/.hermes/state.db` | SQLite | `sessions.id` UUID | `source` 字段 |
| **codex** (OpenAI Codex) | `~/.codex/state_5.sqlite` + `.codex/sessions/*/*.jsonl` | SQLite + JSONL | rollout 文件名 | `source` 字段 |
| **claude** (Claude Code) | `~/.claude/usage-data/session-meta/*.json` + `stats-cache.json` | JSON | 文件名 UUID | CLI vs Desktop 不同目录 |
| **qwen** (Qwen Code) | `~/.qwen/projects/*/chats/*.jsonl` | JSONL | 聊天文件名 | 独立 VS Code 扩展路径 |

## Adapter 模式

每个 CLI 实现一个 `UsageReader` trait。读取逻辑在独立线程中异步执行，通过 channel 发送进度和结果。

```rust
trait UsageReader: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_installed(&self) -> bool;
    fn read(&self, range: &DateRange, progress: &mpsc::Sender<ProgressUpdate>) -> Vec<UsageRecord>;
}
```

```rust
struct UsageRecord {
    session_id: String,        // 来自数据源的真实会话 ID
    cli_name: String,
    source_type: SourceType,   // Cli | Gui | Unknown
    model_name: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
    reasoning_tokens: u64,
    request_count: u64,
    cost_cents: u64,
    date: NaiveDate,
    hour: u8,
    timestamp: NaiveDateTime,
}
```

## 加载与并发机制

```
用户打开 GUI（默认当天）
    │
    ├── N 个并行线程（每个已安装 CLI 一个）
    │
    ├── Thread A: opencode ─── SQLite 查询 ─── 完成
    ├── Thread B: qwen ─────── JSONL 解析 ─── 完成
    ├── Thread C: claude ───── JSON 读取 ──── 完成
    ├── Thread D: hermes ───── SQLite 查询 ─── 完成
    └── Thread E: codex ────── SQLite + JSONL ─ 完成
                                │
                                ▼
                          ┌──────────┐
                          │ 结果队列  │
                          │ mpsc chan │
                          └─────┬────┘
                                │ poll_channels() 每帧检查
                                ▼
                          ┌──────────┐
                          │ 聚合数据  │
                          │ rebuild  │
                          └──────────┘
```

## GUI 布局

```
┌──────────────────────────────────────────────────────────────┐
│  AI 使用量统计  [今天][本周][本月][自定义]    [↻] [⚙]      │
├──────────────────────────────────────────────────────────────┤
│  [📈 概览]  [工具1]  [工具2]  [工具3] ...                   │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌────────────────────┐  ┌────────────────────┐            │
│  │ 模型用量明细        │  │ 🏆 工具使用排名      │            │
│  │ ────────────────    │  │ ────────────────   │            │
│  │ 工具 模型 请求 ...  │  │ 🥇 #1 opencode      │            │
│  │ opencode ds-v4  45  │  │ 🥈 #2 claude        │            │
│  │ claude  sonnet  12  │  │ 🥉 #3 hermes        │            │
│  └────────────────────┘  └────────────────────┘            │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  小时级 Token 趋势图                                   │   │
│  │  ┌─────────────────────────────────────────────┐    │   │
│  │  │  ╱╲      ╱╲                                  │    │   │
│  │  │ ╱  ╲    ╱  ╲    ╱╲                           │    │   │
│  │  └─────────────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  [工具详情页]                                                 │
│  ┌─ 统计卡 ─────────────────────────────────────────────┐   │
│  │ 💠 总Token: 234K   💿 缓存: 65%  💬 请求: 45  🤖 2模型│   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌─ 模型分包 ───────────────────────────────────────────┐   │
│  │ 模型             请求   Token   输入   输出  缓存   % │   │
│  │ deepseek-v4-flash  45  234,567  200K   34K  187K  80%│   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌─ 每次会话明细 ───────────────────────────────────────┐   │
│  │ 会话 ID           时间        模型          Token   缓存%│   │
│  │ 550e8400-e2…  2026-05-22 14  ds-v4-flash  12,345  76% │   │
│  │ 08fa92c9-38…  2026-05-22 13  ds-v4-flash   8,901  45% │   │
│  │ ...                                                    │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## 关键设计决策

### 1. 多线程并发加载

```rust
fn start_load(&mut self, range: DateRange) {
    for reader in readers {
        let pt = progress_tx.clone();
        let rt = result_tx.clone();
        let rng = range.clone();
        thread::spawn(move || {
            let records = reader.read(&rng, &pt);
            rt.send(ReaderResult { cli_name: reader.name(), records, error: None });
        });
    }
}
```

每个 reader 在独立线程中运行，通过 `mpsc::channel` 向主线程发送进度和结果。主线程每帧调用 `poll_channels()` 检查并更新 UI。

### 2. 增量聚合

```rust
fn rebuild_aggregates(&mut self) {
    self.cli_summaries = aggregate_by_cli(&self.all_records);
    self.model_breakdowns = aggregate_by_model(&self.all_records);
    self.hourly_data = aggregate_hourly(&self.all_records);
}
```

三个聚合维度：按工具汇总、按模型汇总、按小时汇总。每次新结果到达后重新计算。

### 3. 会话 ID 追踪

从各工具的数据源提取真实会话标识：

| 工具 | 会话 ID | 实现 |
|------|---------|------|
| opencode | `session.name` (UUID) | SQL 加 `name` 列 |
| hermes | `sessions.id` (UUID) | SQL 加 `id` 列 |
| codex | `rollout_path` 文件名 | 文件路径 `file_stem()` |
| claude | `session-meta/*.json` 文件名 | `path.file_stem()` |
| qwen | `chats/*.jsonl` 文件名 | `path.file_stem()`

### 4. 时间范围选择

```rust
fn time_range_selector(&mut self, ui: &mut egui::Ui, c: &ThemeColors) {
    // 水平按钮行：[今天][本周][本月][自定义]
    // Custom 模式下追加：[2026-05-01] -> [2026-05-22] [应用]
    // 弹出日历选择器 (egui::Area)
}
```

自定义日期使用弹出式日历（`egui::Area`），支持月份翻页、今日高亮。

### 5. 缓存读取耗时优化

| 工具 | 主要耗时 | 优化策略 |
|------|---------|---------|
| opencode | SQLite 1 次查询 | ✅ 快 |
| hermes | SQLite 1 次查询 | ✅ 快 |
| codex | SQLite 快 + JSONL 需额外解析 | 先从 SQLite 读 `tokens_used`，后台懒加载 JSONL |
| claude | 读取 ~73 个 JSON 文件 | 并行读取 |
| qwen | 扫描大量 JSONL 文件 | 逐行解析 |

## 项目结构

```
ai-usage-statistics/
├── Cargo.toml
├── build/
│   └── macos/
│       ├── Info.plist          # .app bundle 元信息
│       └── AppIcon.icns        # 应用图标 (1024x1024, dark neon chart)
├── icons/
│   ├── opencode.png            # 工具头像
│   ├── claude.png
│   ├── hermes.png
│   ├── codex.png
│   └── qwen.png
├── src/
│   ├── main.rs                 # 入口 + 全部 UI 逻辑 (~1600 行)
│   ├── models.rs               # UsageRecord, 聚合函数, 格式化
│   ├── readers.rs              # 5 个 UsageReader 实现 (~740 行)
│   └── updater.rs              # 自动更新
├── .github/workflows/
│   └── release.yml             # 跨平台构建 + 发布
├── README.md
└── DESIGN.md
```

## 各 Adapter 实现要点

### opencode

```sql
SELECT tokens_input, tokens_output, tokens_cache_read,
       tokens_cache_write, tokens_reasoning, model, time_created,
       cost, agent, name
FROM session
WHERE date(time_created / 1000, 'unixepoch') BETWEEN ?1 AND ?2
ORDER BY time_created;
```

### hermes

```sql
SELECT model, input_tokens, output_tokens, cache_read_tokens,
       cache_write_tokens, reasoning_tokens, started_at,
       estimated_cost_usd, actual_cost_usd, source, id
FROM sessions
WHERE started_at BETWEEN ?1 AND ?2
ORDER BY started_at;
```

### codex

```sql
SELECT model, tokens_used, source, created_at, rollout_path
FROM threads
WHERE created_at BETWEEN ?1 AND ?2
ORDER BY created_at;
```

JSONL 中解析 `input_token_details.cached_tokens` 或 `usageMetadata.cachedContentTokenCount`。

### qwen

```rust
// 遍历 ~/.qwen/projects/*/chats/*.jsonl
// 每行 JSON，筛选 date 范围内的行
// 提取 usageMetadata.promptTokenCount / candidatesTokenCount / totalTokenCount / cachedContentTokenCount
// 或 ui_telemetry 事件的 input/output/cached_token_count
```

### claude

```rust
// 读取 ~/.claude/usage-data/session-meta/*.json
// 每个文件一个 session，文件名即会话 ID
// input_tokens, output_tokens, model, start_time
// stats-cache.json 提供模型级的缓存聚合值
```

## 技术要点

1. **跨平台数据路径**：通过 `dirs`/`dirs-next` crate 统一处理各平台数据目录差异
2. **只读访问**：所有 adapter 以只读方式操作，不修改任何数据
3. **会话 ID 提取**：尽可能从源数据获取真实标识（UUID），便于核对
4. **优雅降级**：某个工具读取失败不影响其他工具，错误信息单独展示
5. **图标嵌入**：编译时 `include_bytes!` 将 `AppIcon.icns` 嵌入二进制，解析为 RGBA 设置到窗口
6. **自定义日期**：弹出日历控件，支持月份翻页、选择后即时更新，需点击"应用"触发加载
7. **工具排名**：首页模型明细右侧显示按总 Token 降序的工具排名，含排名图标和进度条
