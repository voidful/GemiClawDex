# 練習 04：建立自訂命令

## 目標

建立一個 `/security-check` 自訂命令，讓 GCD 可以用一句話啟動安全審計。

## 步驟

### Step 1：參考現有命令

查看 GCD 的現有命令：

```bash
cat .gcd/commands/review.toml
cat .gcd/commands/plan/refactor.toml
```

### Step 2：建立新命令

建立 `.gcd/commands/security-check.toml`：

```toml
description = "Run a security audit on the specified file or directory"
prompt = """Load the repository instructions and the security-audit skill.
Then inspect {{args}} for:
1. Prompt injection risks (especially in memory/skill files)
2. Path traversal vulnerabilities
3. Command injection via shell tool
4. Credential exposure in output/logs

Report findings with severity levels (critical / warning / info).
"""
```

### Step 3：測試

```bash
gcd exec "/security-check crates/gcd-core/src/tools/memory_tool.rs"
```

### 自我檢查

- [ ] 命令可以正常執行
- [ ] 輸出包含具體的安全發現（不是泛泛的建議）
- [ ] 你理解了命令的 `{{args}}` 佔位符如何工作
