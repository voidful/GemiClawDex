# 練習 03：Context 注入對比實驗

## 目標

親眼看到「送什麼 context 進去」如何影響 AI 的輸出品質。

## 步驟

在 GCD 專案目錄下，執行以下三個變體：

### 變體 A：只有 prompt

```bash
gcd exec "這個專案的 trust 系統是怎麼設計的"
```

記錄：回答的具體程度、有沒有引用實際程式碼、token 用量。

### 變體 B：加上檔案注入

```bash
gcd exec "分析 @{crates/gcd-core/src/trust.rs} 的 trust 系統設計"
```

記錄：同上。

### 變體 C：加上 AGENTS.md + 檔案注入

確認 AGENTS.md 存在後：

```bash
gcd exec "分析 @{crates/gcd-core/src/trust.rs} 的 trust 系統設計，以及它和 permission 系統的關係"
```

### Step 4：比較三組結果

填寫以下表格：

| 維度 | 變體 A | 變體 B | 變體 C |
|------|--------|--------|--------|
| 回答是否引用具體程式碼 | | | |
| 是否提到設計來源（Claude Code / Codex） | | | |
| Token 用量 | | | |
| 你覺得有用的程度（1-5） | | | |

## 預期發現

- 變體 A 通常回答泛泛而談
- 變體 B 能引用具體程式碼但缺少設計脈絡
- 變體 C 能同時提供程式碼細節和設計理由
