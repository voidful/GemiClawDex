# 貢獻指南 / Contributing Guide

感謝你對 GemiClawDex 的興趣！這是一個教學專案，歡迎各種形式的貢獻。

## 貢獻方式

### 🟢 最容易開始的貢獻

- **修正錯字或翻譯問題**：文件全站繁體中文，如果發現用詞不自然或簡體用語，歡迎提 PR。
- **補充練習題**：在 `exercises/` 目錄新增動手練習。
- **新增 Skill**：在 `.gcd/skills/` 目錄新增可重用技能，格式參考 `code-review/SKILL.md`。

### 🟡 中等難度

- **新增教學章節**：在 `docs/chapters/` 新增 HTML 章節，並更新 `manifest.json`。
- **改善現有章節**：補充原始碼對照、修正文實落差。
- **英文翻譯**：目前文件以繁中為主，歡迎翻譯重要章節。

### 🔴 需要 Rust 經驗

- **新增工具**：在 `crates/gcd-core/src/tools/` 實作新的 Tool trait。
- **改善 Provider 適配**：在 `crates/gcd-core/src/agent/adapters/` 改善 API 格式處理。
- **寫測試**：為現有功能補充單元測試。

## 開發環境設定

```bash
# 克隆
git clone https://github.com/voidful/GemiClawDex.git
cd GemiClawDex

# 編譯
cargo build

# 測試
cargo test

# Lint
cargo clippy
cargo fmt --check

# 預覽文件網站
python3 -m http.server 8000 --directory docs
```

或使用 GitHub Codespaces（零設定）：點 repo 頁面的 "Code" → "Codespaces" → "Create codespace"。

## Skill 貢獻格式

新增 Skill 時請遵循以下格式（與 [Hermes Agent](https://github.com/NousResearch/hermes-agent) 相容）：

```markdown
---
name: your-skill-name
description: "一句話描述何時使用這個 skill"
version: 1.0.0
author: Your Name
license: Apache-2.0
metadata:
  gcd:
    tags: [tag1, tag2]
    related_skills: [existing-skill-name]
---

# Skill Title

## Core Principle
一句話核心原則。

## When to Use
何時啟用。

## Process
具體步驟。
```

## 教學章節貢獻格式

新增章節時：

1. 在 `docs/chapters/` 建立 `your-chapter-id.html`。
2. 使用現有章節的 HTML 結構（`chapter-header` → `content-body` → `chapter-nav`）。
3. 在 `docs/chapters/manifest.json` 的適當位置新增導航項。
4. 確保章節包含 `source-ref` 標註對應的原始碼路徑。

## PR 規範

- 一個 PR 做一件事。不要混合文件修改和程式碼修改。
- 如果修改了 Rust 程式碼，確保 `cargo test` 和 `cargo clippy` 都通過。
- 如果新增了教學內容，確保文實相符：不要寫「已完成」的功能描述來形容還在規劃中的特性。

## 授權

所有貢獻都會以 Apache-2.0 授權釋出。
