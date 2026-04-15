# GemiClawDex (GCD)

> 用 Rust 從頭建構 AI coding agent，學會 Harness Engineering。
>
> *Learn Harness Engineering by building an AI coding agent from scratch in Rust.*
>
> **[English README](README.en.md)**

<p align="center">
  <a href="http://eric-lam.com/GemiClawDex/"><img src="https://img.shields.io/badge/教學網站-互動式文件-FFD700?style=for-the-badge" alt="Docs"></a>
  <a href="https://github.com/voidful/GemiClawDex/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-Apache--2.0-green?style=for-the-badge" alt="License"></a>
</p>

## 這個專案在做什麼

你可能用過 Claude Code、Gemini CLI、OpenAI Codex 來寫程式。但你有沒有想過：**它們背後是怎麼運作的？**

GCD 是一個用 Rust 寫的 AI coding agent。它不是要取代上面三個工具，而是把它們的設計精華拆開來，讓你透過閱讀原始碼和互動式文件，理解 AI 助手的「外殼」是怎麼做出來的。

這層外殼叫做 **Harness**。它負責組合提示詞、管理工具、控制權限、記錄工作過程、對接不同的 AI 模型。AI 模型是引擎，Harness 是方向盤、煞車和儀表板。

> **What is this?** GCD is a Rust-native AI coding agent that combines design patterns from Claude Code, Gemini CLI, and OpenAI Codex. It is a teaching project. Every design decision is traceable to its source, and every module maps to a chapter in the [interactive documentation](http://eric-lam.com/GemiClawDex/).

## 為什麼在這裡學

| 特點 | 說明 |
|------|------|
| **不是講解別人的產品** | GCD 本身就是產品，13,000+ 行 Rust 原始碼完全公開可讀 |
| **不綁定單一廠商** | 同時支援 Gemini / OpenAI / Anthropic / OpenRouter / 本地模型 |
| **融合三家設計** | 每個設計決策都標註來自哪個產品、為什麼這樣取捨 |
| **有互動式教學網站** | 25+ 個主題章節，從入門到原始碼對照 |
| **技能學習迴圈** | 參考 [Hermes Agent](https://github.com/NousResearch/hermes-agent) 的 skill 系統，agent 能從經驗中建立可重用技能 |
| **雙重記憶系統** | MEMORY.md（環境知識）+ USER.md（使用者偏好），含安全掃描防注入 |

## 教學網站

🌐 **[eric-lam.com/GemiClawDex](http://eric-lam.com/GemiClawDex/)**

不需要先會 Rust。文件本身就是獨立的 Harness Engineering 學習資源。

### 學習路徑

| 時間 | 路徑 | 適合誰 |
|------|------|--------|
| 3 分鐘 | 首頁 → 前言 → 入門範例 | 第一次接觸 agent 概念 |
| 10 分鐘 | + 術語速查 → GCD 全景 | 想搞懂 prompt / context / harness 三者的差異 |
| 30 分鐘 | + Pipeline → 各細節章節 | 想完整理解從輸入到執行的流程 |
| 1 小時 | + 對照章 → 原始碼 | 準備動手改 code 或做自己的 agent |
| 動手做 | AGENTS.md 工作坊 + 練習題 | 想為自己的專案寫 harness |

## 從哪個產品學了什麼

| 來源 | 學到的設計 | 對應原始碼 |
|------|-----------|-----------|
| **Claude Code** | Prompt 組裝、Tool trait、Trust 邊界、Skill 系統 | `prompt.rs`, `tools.rs`, `trust.rs`, `skills.rs` |
| **Gemini CLI** | Terminal-first REPL、MCP 客戶端、Token Caching、Streaming | `main.rs`, `mcp.rs`, `cache.rs`, `output.rs` |
| **OpenAI Codex** | Sandbox 分級、Permission 三級制、apply-patch | `tools/container.rs`, `agent/permissions.rs`, `tools/apply_patch.rs` |
| **Hermes Agent** | 技能學習迴圈、雙重記憶、記憶安全掃描、Session 搜尋 | `skills.rs`, `tools/memory_tool.rs`, `tools/skill_manager.rs` |

## 快速開始

```bash
# 編譯
git clone https://github.com/voidful/GemiClawDex.git && cd GemiClawDex
cargo build --release

# 設定 API 金鑰（至少一個）
export GEMINI_API_KEY="AIza..."      # 免費額度最高

# 啟動互動式 REPL
./target/release/gcd

# 或執行單次任務
gcd exec "解釋這個 codebase 的架構"
```

更多用法請參考教學網站的[安裝指南](http://eric-lam.com/GemiClawDex/)和[互動模式指南](http://eric-lam.com/GemiClawDex/)。

## 架構

```
crates/
├── gcd-core/          # 核心邏輯庫（~12,000 行）
│   ├── agent.rs       # Agent 執行迴圈 + Permission + Streaming + Memory
│   ├── tools/         # 11 個內建工具 + coordinator（930 行 DAG 排程）
│   ├── providers.rs   # 多供應商管理（Gemini / OpenAI / Anthropic）
│   ├── prompt.rs      # Prompt 組裝引擎
│   ├── session.rs     # Session 持久化
│   ├── trust.rs       # 三級信任模型
│   ├── skills.rs      # 技能系統 + YAML frontmatter + Progressive Disclosure
│   ├── mcp.rs         # MCP 客戶端
│   └── hooks.rs       # PreToolUse / PostToolUse 生命週期鉤子
├── gcd-cli/           # CLI 入口
│   └── main.rs        # clap 4 + rustyline REPL
```

## 內建技能

`.gcd/skills/` 目錄包含可重用的 agent 技能（YAML frontmatter 格式，與 Hermes Agent 相容）：

```
.gcd/skills/
├── code-review/SKILL.md           # 程式碼審查
├── systematic-debugging/SKILL.md  # 系統化除錯（四階段根因分析）
├── tdd/SKILL.md                   # 測試驅動開發（RED-GREEN-REFACTOR）
├── writing-plans/SKILL.md         # 實作計畫撰寫
├── security-audit/SKILL.md        # 安全性審計
├── refactoring/SKILL.md           # 重構指南
├── documentation-review/SKILL.md  # 文件審查（文實相符檢查）
├── git-workflow/SKILL.md          # Git 工作流（原子 commit）
├── performance-analysis/SKILL.md  # 效能分析（含 token 效率）
└── architecture-review/SKILL.md   # 架構審查（依賴方向、層次違反）
```

## Permission 模型

| 等級 | write_file | shell | apply_patch | 說明 |
|------|------------|-------|-------------|------|
| `suggest` | ⚠️ 需確認 | ⚠️ 需確認 | ⚠️ 需確認 | 最安全 |
| `auto-edit` | ✅ 自動 | ⚠️ 需確認 | ⚠️ 需確認 | 預設值 |
| `full-auto` | ✅ 自動 | ✅ 自動 | ✅ 自動 | 僅限信任環境 |

## 設計靈感

| 來源 | 採納特性 |
|------|---------|
| [Gemini CLI](https://github.com/google-gemini/gemini-cli) | REPL 互動、Streaming、MCP、Token Caching |
| [OpenAI Codex](https://github.com/openai/codex) | Sandbox 分級、Permission 模型、apply-patch |
| [Claude Code](https://github.com/roger2ai/Claude-Code-Compiled) | Trust 邊界、Session 分支、Skill 系統 |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent) | 技能學習迴圈、雙重記憶、記憶安全掃描、Session 搜尋、Progressive Skill Disclosure |

## 參考資源

- [Martin Fowler — Harness Engineering](https://martinfowler.com/articles/harness-engineering.html)
- [Anthropic — Effective harnesses for long-running agents](https://docs.anthropic.com)
- [OpenAI — Harness engineering: leveraging Codex](https://openai.com)

## 授權

Apache-2.0

---

<p align="center">
  <sub>Built as a teaching project by <a href="https://github.com/voidful">@voidful</a>. 不是要取代 Claude Code / Gemini CLI / Codex，而是要讓你搞懂它們。</sub>
</p>
