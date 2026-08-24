# Issue tracker: Local Markdown

本仓库的需求规格与实施工单均作为 Markdown 文件保存在 `.scratch/`。

## Conventions

- 每个功能使用一个目录：`.scratch/<feature-slug>/`。
- 规格文件为 `.scratch/<feature-slug>/spec.md`。
- 实施工单分别保存为 `.scratch/<feature-slug>/issues/<NN>-<slug>.md`，从 `01` 起按依赖顺序编号；不得合并成单一 tickets 文件。
- Triage 状态在每个工单顶部附近使用 `Status:` 字段记录；角色字符串见 `triage-labels.md`。
- 评论与讨论记录追加到文件底部的 `## Comments` 小节。

## Skill 中的发布与读取

- 当 Skill 要求“发布到 issue tracker”时，只在 `.scratch/<feature-slug>/` 创建或更新本地文件，不执行远程写入。
- 当 Skill 要求读取工单时，读取用户指定路径或编号对应的本地文件。

## Wayfinding operations

- Map：`.scratch/<effort>/map.md`。
- 子工单：`.scratch/<effort>/issues/NN-<slug>.md`。
- `Blocked by:` 记录阻塞编号；所有阻塞工单为 `resolved` 后才解除阻塞。
- Frontier 是状态为 open、未被阻塞且未被认领的最小编号工单。
- 认领时将 `Status:` 改为 `claimed`；解决时追加 `## Answer`，将状态改为 `resolved`，并把上下文指针写回 map。
