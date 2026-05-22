# AI Usage Statistics

一款用于查看 AI 命令行工具使用统计的桌面应用。基于 Rust + egui 构建。

## 截图

（待添加）

## 支持的 AI 工具

| 工具 | 来源 |
|---|---|
| [opencode](https://opencode.ai) | SQLite 数据库 `~/.local/share/opencode/opencode.db` |
| [Hermes](https://nousresearch.com) | SQLite 数据库 `~/.hermes/state.db` |
| [Codex CLI](https://openai.com) | SQLite 数据库 `~/.codex/state_5.sqlite` |
| [Claude Code](https://anthropic.com) | SQLite 数据库 `~/.claude/` |
| [Qwen](https://qwen.alibaba.com) | SQLite 数据库 `~/.qwen/` |

## 功能

- **数据概览面板**：展示总 Token 数、缓存命中率、总请求数、活跃工具数
- **工具详情页**：按工具分页展示详细的模型用量、输入/输出 Token、缓存情况
- **模型用量明细表**：跨工具整合，展示每个模型的请求数、Token 数、缓存率、费用
- **小时级趋势图**：折线图展示全天各小时的 Token 走势（含缓存命中）
- **时间范围筛选**：今日 / 本周 / 本月 / 自定义日期
- **深色/浅色主题**：一键切换
- **中英文双语**：支持中文 / English 界面
- **工具排序**：自定义工具的显示顺序
- **路径覆盖**：可按需覆盖各工具的数据库路径

## 架构

```
┌─────────────────────────────────┐
│           eframe 窗口           │
│  ┌───────────────────────────┐  │
│  │     egui UI 层            │  │
│  │  - 摘要面板               │  │
│  │  - 工具标签页             │  │
│  │  - 模型表格 / 趋势图      │  │
│  │  - 设置弹窗               │  │
│  └──────────┬────────────────┘  │
│             │                   │
│  ┌──────────▼────────────────┐  │
│  │     Reader 层             │  │
│  │  - opencode 读取器        │  │
│  │  - hermes 读取器          │  │
│  │  - codex 读取器           │  │
│  │  - claude 读取器          │  │
│  │  - qwen 读取器            │  │
│  └──────────┬────────────────┘  │
│             │                   │
│  ┌──────────▼────────────────┐  │
│  │     SQLite（rusqlite）    │  │
│  │  各工具本地数据库文件     │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

- **UI 层**：egui + egui_plot 实现即时模式 GUI
- **数据层**：Reader trait，每个工具一个实现，通过多线程并发读取
- **存储层**：直接读取各工具的本地 SQLite 数据库，不依赖任何 API
- **配置持久化**：JSON 文件存储主题、语言、工具顺序、路径覆盖

## 安装环境

### 前置条件

- [Rust](https://www.rust-lang.org/) 工具链（最低支持 1.80+）

### 编译运行

```bash
# 克隆仓库
git clone https://github.com/nontracey/AIUsageStatistics.git
cd AIUsageStatistics

# 运行（开发模式）
cargo run

# 构建发布版
cargo build --release
```

编译产物位于 `target/release/ai-usage-statistics`，可直接分发运行。

## 使用方式

启动后，应用会自动检测系统中已安装的 AI 工具的数据库，并读取指定时间范围内的使用记录。

### 时间范围

点击顶部栏的「今日 / 本周 / 本月 / 自定义」切换时间范围，首次选择会自动触发加载。

### 自定义日期

选择「自定义」后，可分别点击起止日期打开日历选择器，点击「应用」加载数据。

### 切换视图

- **概览标签页**：展示全局统计数据、各工具用量分栏、模型用量明细表、小时级趋势图
- **工具标签页**：点击顶部工具按钮进入对应工具的详细视图

### 设置

点击右上角 ⚙ 进入设置：
- 切换深色/浅色主题
- 切换中文/English 语言
- 调整工具显示顺序（上下按钮拖动）
- 覆盖工具数据库路径

### 配置存储

设置保存在：
- macOS：`~/Library/Application Support/ai-usage-stats/config.json`
- Linux：`~/.config/ai-usage-stats/config.json`

格式示例：
```json
{
  "theme": "Dark",
  "language": "Chinese",
  "tool_order": ["opencode", "claude", "codex", "hermes", "qwen"],
  "tool_path_overrides": {}
}
```

## 技术栈

- **语言**：Rust 2021 edition
- **GUI 框架**：egui 0.31 + eframe 0.31
- **图表**：egui_plot 0.31
- **数据库**：rusqlite 0.31 (bundled SQLite)
- **图像处理**：image 0.25
- **序列化**：serde / serde_json
- **时间处理**：chrono 0.4

## 许可

MIT
