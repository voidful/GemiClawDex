# 練習 02：為三種不同專案撰寫 AGENTS.md

## 目標

練習用「五個原則」為不同類型的專案撰寫有效的 AGENTS.md。

## 步驟

### Step 1：閱讀教學網站的「AGENTS.md 撰寫工作坊」章節

回顧五個原則：告訴 agent 你在哪、不要做什麼、怎麼驗證、保持短小、分層放置。

### Step 2：為以下三個假設專案各寫一份 AGENTS.md

**專案 A：個人部落格**
- 技術棧：Next.js 14 + MDX + Tailwind
- 部署：Vercel
- 你的限制：不想用任何外部 CMS

**專案 B：REST API 後端**
- 技術棧：Go 1.22 + Chi router + PostgreSQL
- 規範：所有 endpoint 都要有 OpenAPI 文件
- 你的限制：不能使用 ORM，只用 raw SQL

**專案 C：機器學習實驗**
- 技術棧：Python 3.11 + PyTorch + Weights & Biases
- 規範：所有實驗必須可重現（固定 seed）
- 你的限制：資料集不能 commit 到 git

### Step 3：自我評估

用以下 checklist 評估你寫的每份 AGENTS.md：

- [ ] 有沒有告訴 agent 專案結構和關鍵路徑？
- [ ] 有沒有至少一條「不要做什麼」的禁止項？
- [ ] 有沒有提供驗證指令（test / lint / build）？
- [ ] 是否在 500 字以內？
- [ ] 讀起來像是在對一個新加入的工程師說明，而不是寫技術文件？

## 參考答案

參考答案在教學網站的「AGENTS.md 撰寫工作坊」章節中有三個完整範例。
