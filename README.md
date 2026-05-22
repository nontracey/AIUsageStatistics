# AI Usage Statistics

A desktop app for viewing AI CLI tool usage statistics. Built with Rust + egui.

## Supported Tools

- [opencode](https://opencode.ai)
- [Hermes](https://nousresearch.com)
- [Codex CLI](https://openai.com)
- [Claude Code](https://anthropic.com)
- [Qwen](https://qwen.alibaba.com)

## Features

- Summary dashboard with token usage, cache hit rate, request counts
- Per-tool detailed breakdown by model
- Hourly token usage trend chart
- Date range filter (today / week / month / custom)
- Dark & Light themes
- Chinese / English i18n
- Customizable tool order and path overrides

## Usage

```bash
cargo run
```

## Build

```bash
cargo build --release
```

## Configuration

Settings are saved to:
- macOS: `~/Library/Application Support/ai-usage-stats/config.json`
- Linux: `~/.config/ai-usage-stats/config.json`

## Data Sources

Each tool's local SQLite database is read directly:
- opencode: `~/.local/share/opencode/opencode.db`
- hermes: `~/.hermes/state.db`
- codex: `~/.codex/state_5.sqlite`
- claude: `~/.claude/`
- qwen: `~/.qwen/`
