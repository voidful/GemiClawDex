# 練習 05：建立可重用 Skill

## 目標

建立一個 `documentation-review` skill，讓 agent 在審查文件時遵循固定的品質標準。

## 步驟

### Step 1：了解 Skill 格式

查看現有 skill 的結構：

```bash
cat .gcd/skills/code-review/SKILL.md
cat .gcd/skills/systematic-debugging/SKILL.md
```

注意 YAML frontmatter 的欄位：`name`, `description`, `version`, `author`, `license`, `metadata.gcd.tags`, `metadata.gcd.related_skills`。

### Step 2：建立新 Skill

建立 `.gcd/skills/documentation-review/SKILL.md`，至少包含：

1. **YAML frontmatter**：完整的 metadata
2. **Core Principle**：一句話說明這個 skill 的核心原則
3. **When to Use**：什麼時候應該啟用這個 skill
4. **Checklist**：文件審查的具體檢查項目

建議的檢查項目：
- 文件是否和原始碼一致（文實相符）
- 是否有過時的範例或截圖
- 是否對功能狀態做了誠實標註（Implemented / Partial / Stub）
- 是否有足夠的程式碼範例
- 是否照顧到完全沒有背景知識的讀者

### Step 3：測試 Skill

```bash
gcd exec "使用 documentation-review skill 檢查 docs/chapters/container.html 的文件品質"
```

### 進階挑戰

修改 skill 使其支援不同語言的文件（繁中 / 英文），並在 `metadata.gcd.tags` 中加入 `i18n`。

### 自我檢查

- [ ] YAML frontmatter 格式正確
- [ ] Skill 有明確的 Core Principle
- [ ] 檢查項目是具體的（不是泛泛的「確保品質好」）
- [ ] Agent 執行時確實引用了 skill 中的檢查項目
- [ ] `related_skills` 欄位正確引用了其他 skill
