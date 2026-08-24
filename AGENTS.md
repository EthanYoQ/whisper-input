# Whisper Input 源码规则

## Agent skills

### Issue tracker

需求规格与实施工单使用仓库内 `.scratch/` 的本地 Markdown 文件。参见 `docs/agents/issue-tracker.md`。

### Triage labels

使用五个默认 MATT triage 标签。参见 `docs/agents/triage-labels.md`。

### Domain docs

本仓库使用单一上下文布局：根目录 `CONTEXT.md` 与 `docs/adr/`。参见 `docs/agents/domain.md`。

## Right Alt / AltGr 热键回归规则

- Right Alt 不能录入、不能触发、或默认右 Alt 按住说话失效，是本项目的高风险重复回归；处理时必须先看真实日志和 Windows 低级键盘 hook 事件流，不能只看前端录制组件或后端分发后的 mock 事件。
- 重点检查事件进入 `dispatch_keyboard_event` 之前的准入层，尤其是 `LLKHF_INJECTED`、`LLKHF_EXTENDED`、`VK_MENU`、`VK_RMENU`、AltGr/Right Alt 归一化路径。过去的问题是 Right Alt/AltGr 被 Windows 报告为 injected Alt 事件后，在进入录制器前就被过滤，导致测试通过但实机无法录入。
- 修复必须保持按住说话语义；不要把默认模式从 Hold 改成 Toggle 来绕过问题。除非有证据证明需求变化，否则 Right Alt 默认仍应是 Hold。
- 测试必须覆盖 hook 事件准入层，而不仅是“已经进入分发后能映射为 `AltRight`”。至少保留/运行 CI-safe 核心测试，验证 injected Alt-family 事件会被接受，普通 injected 非 Alt 键仍会被过滤。
- 交付前必须运行 `npm run check:hotkey-injection` 和 `$env:GITHUB_ACTIONS='true'; npm run check:hotkey-injection`。只有执行发布时，才需要确认 GitHub Actions 的 `Hotkey regression` 成功、Release 非 draft/非 prerelease 且安装包资产已上传。

## UI 验证规则

- 前端视觉、布局或样式改动必须通过真实浏览器或应用截图验证关键页面；确认无明显错位、重叠或截断后，才能报告完成。
