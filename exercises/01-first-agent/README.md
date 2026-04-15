# 練習 01：第一次使用 Agent

## 目標

理解 AI 助手和普通聊天機器人的差異。觀察 agent loop 的「工具呼叫 → 執行 → 回傳結果」循環。

## 前提

- GCD 已編譯（`cargo build --release`）
- 至少一個 API 金鑰已設定

## 步驟

### Step 1：確認 GCD 可以啟動

```bash
gcd overview
gcd providers doctor
```

如果看到至少一個 provider 顯示 OK，就可以繼續。

### Step 2：執行一個需要工具的任務

```bash
cd /path/to/GemiClawDex   # 進入 GCD 自己的專案目錄
gcd exec "列出 crates/gcd-core/src/ 底下所有 .rs 檔案，並說明每個檔案的用途"
```

**觀察重點**：
- Agent 是否呼叫了 `list_dir` 或 `read_file` 工具？
- Agent 是否自己讀取了檔案內容再回答，而不是靠猜測？
- 輸出中有沒有顯示 token 用量？

### Step 3：對比一下「不用工具」的結果

在 ChatGPT 或其他聊天介面中問同樣的問題：「列出 GemiClawDex 的 crates/gcd-core/src/ 底下所有 .rs 檔案，並說明每個檔案的用途」。

**差異**：聊天機器人只能根據訓練資料猜測，無法真正讀取你的檔案。GCD 會呼叫工具去讀，然後基於真實內容回答。

## 預期結果

你應該看到 GCD 的輸出中包含：
- 實際的檔案列表（agent.rs, tools.rs, prompt.rs 等）
- 每個檔案的功能描述（基於實際讀取的內容）
- Session 結束時的 token 用量統計

## 自我檢查

- [ ] 我觀察到了至少一次工具呼叫
- [ ] Agent 的回答是基於實際檔案內容，不是泛用猜測
- [ ] 我理解了「agent = model + harness」的意思
