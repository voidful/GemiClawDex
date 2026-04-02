# GemiClawdex

> **高效能終端 AI 編碼代理** — 融合 Gemini CLI、OpenAI Codex 與 Claude Code 的設計精髓，以 Rust 打造。

## 特色

- 🦀 **Rust 原生** — 編譯為單一二進位檔 `gcd`，啟動迅速、記憶體安全
- 🤖 **Agent Loop** — 真正的 LLM 工具迴圈：送出 → 回覆 → 工具呼叫 → 執行 → 迴圈
- 🔧 **內建工具** — read_file, write_file, list_dir, shell, search_files
- 🎯 **多供應商** — 同時支援 Gemini、OpenAI、Anthropic API
- 🛡️ **Sandbox 策略** — 四級安全模型：off / read-only / workspace-write / container
- 🔒 **Trust 系統** — 工作區信任邊界管理，防止未授權操作
- 📝 **Session 持久化** — 自動保存對話歷史，支援 resume / fork
- 💻 **互動式 REPL** — 無參數啟動即進入終端對話模式

## 快速開始

```bash
# 編譯
cargo build --release

# 互動模式（直接啟動 REPL）
./target/release/gcd

# 執行單次任務
gcd exec "解釋這個 codebase 的架構"

# 指定供應商
gcd exec --provider gemini-env "寫一個測試"

# 檢視工作區概述
gcd overview

# 管理供應商
gcd providers list
gcd providers doctor

# JSON 輸出
gcd overview --json
```

## 環境變數

| 變數 | 說明 |
|------|------|
| `GEMINI_API_KEY` | Google Gemini API 金鑰 |
| `OPENAI_API_KEY` | OpenAI API 金鑰 |
| `ANTHROPIC_API_KEY` | Anthropic API 金鑰 |
| `GEMICLAWDEX_PROVIDER` | 預設供應商 ID |
| `GEMICLAWDEX_SANDBOX` | 預設 sandbox 策略 |

## 架構

```
crates/
├── gemi-clawdex-core/      # 核心邏輯庫
│   ├── agent.rs             # Agent 執行迴圈（API 呼叫 + 工具分派）
│   ├── tools.rs             # Tool trait + 5 個內建工具
│   ├── app.rs               # 命令路由 facade
│   ├── providers.rs         # 多供應商管理（Gemini/OpenAI/Anthropic）
│   ├── prompt.rs            # Prompt 組裝（指令注入、檔案注入、命令展開）
│   ├── session.rs           # Session 持久化（建立/恢復/分支）
│   ├── trust.rs             # 工作區信任邊界
│   ├── config.rs            # 路徑檢測與偏好設定
│   ├── output.rs            # 輸出渲染（serde Serialize 驅動）
│   └── ...
├── gemi-clawdex-cli/        # CLI 入口 → 二進位名稱: gcd
│   └── main.rs              # clap 4 + REPL 互動模式
```

## 設計靈感

| 來源 | 採納特性 |
|------|----------|
| [gemini-cli](https://github.com/google-gemini/gemini-cli) | REPL 互動、工具系統、GEMINI.md 上下文 |
| [openai/codex](https://github.com/openai/codex) | Sandbox 分級、codex-rs Rust 實作 |
| [Claude Code](https://github.com/roger2ai/Claude-Code-Compiled) | Trust 邊界、Session 分支、Skill 系統 |
| [claurst](https://github.com/Kuberwastaken/claurst) | Rust 重寫方法論 |

## 授權

Apache-2.0
