# TYPOGRAPHY.md — Whisper Input 字体排版台账

> 依据 `.claude/skills/typography`(jelaludo/claude-skill-typography)§7 要求建立,append-only。
> 每轮涉及字体/字号/行高/字重的改动必须追加 Session Log,并同步更新 Current State。

## Current State(2026-09-04 美化 pass 后)

### 字体栈(tokens.css `--ol-font-sans`)

| 顺位 | 字体 | 角色 |
|---|---|---|
| 1 | Inter(自托管可变字体) | Latin/数字主力 |
| 2 | system-ui | 系统回退 |
| 3 | PingFang SC | macOS/iOS 中文 |
| 4 | HarmonyOS Sans SC | 华为系中文 |
| 5 | MiSans | 小米/部分安卓中文 |
| 6 | Microsoft YaHei UI / YaHei | Windows 中文 |
| 7 | Noto Sans SC | 跨平台中文兜底 |
| 8 | -apple-system / BlinkMacSystemFont / Segoe UI | 旧系统回退 |

- 自托管文件:`src/assets/fonts/inter-var-latin.woff2`,73 KB,可变字重 100–900,latin 子集(unicode-range 限定),符合 skill §6 自托管 <100 KB 预算。
- `font-display: swap`(§6)。
- body 开启 `font-optical-sizing: auto` + `font-feature-settings: 'cv11','ss01','ss03'`。

### 字阶(6 档,token 定义于 tokens.css,§1/§2)

| Token | 值 | 用途 |
|---|---|---|
| `--ol-fs-xs` | 12px | 辅助说明、徽章、表格次级信息(最小可读字号,§3) |
| `--ol-fs-sm` | 13px | 正文、列表、设置行 |
| `--ol-fs-md` | 14px | body 基准 |
| `--ol-fs-lg` | 16px | 卡片标题、强调数值前缀 |
| `--ol-fs-xl` | 22px | 页面级标题(h1/h2),line-height 1.25(§3 标题档) |
| `--ol-fs-num` | 24px | 统计大数字(配合 tabular-nums,§5) |

### 字重(4 档,§4 上限)

| Token | 值 | 用途 |
|---|---|---|
| `--ol-fw-regular` | 400 | 正文 |
| `--ol-fw-medium` | 500 | 列表项、芯片 |
| `--ol-fw-semibold` | 600 | 卡片标题、按钮、强调行 |
| `--ol-fw-bold` | 700 | 页面标题、统计数字 |

### 行高(§3)

- 正文(CJK):body 1.55;长段落 1.6–1.7(中文按 1.7 系取档)。
- 标题:1.25(页标题档)。
- 说明/次级:1.5。

### 数字排印

统计数字列使用 `font-variant-numeric: tabular-nums`(§5),避免等宽跳动。

## Session Log

### 2026-09-04 美化 pass(typography + ui-design skill 联合执行)

**审计基线(改动前)**
- font-size 19 个不同值:11.5×37、12.5×36、11×34 等大量 0.5px 碎档。
- font-weight 10 个值:含 550/650/680/720/750/800 等非标档。
- Google Fonts 外链加载 Inter(阻塞 + 隐私 + 内网不可用)。

**本轮改动**
1. 删除 Google Fonts @import,改自托管 Inter 可变字体 latin 子集(73 KB)— 修复「字体偏丑」主因:此前 latin/数字在 Windows 上大量落回 YaHei 渲染,中西文混排割裂。
2. 字阶 19 → 6 档(12/13/14/16/22/24),CSS 与 TSX 内联样式全量归一。
3. 字重 10 → 4 档(400/500/600/700),550→500、650/680→600、720/750→700、800→700。
4. body line-height 1.55;页标题行高统一 1.25(原 1.15/1.16/1.18 混用)。
5. 词汇表页芯片云卡片限宽 780px 居中(§1.1 阅读容器原则:芯片云无需铺满整张 sheet)。
6. CJK 字体栈现代化:新增 HarmonyOS Sans SC / MiSans / YaHei UI 渐进回退。

**验证**
- 归一后 CSS 字号集合:{12, 13, 14, 16, 22, 24};字重集合:{400, 500, 600, 700}。
- TSX 内联:fontSize {12×105, 13×75, 14×6, 16×5, 22×2},fontWeight {500×43, 600×25, 700×4}。
- `font-weight: 100 900` 仅存于 @font-face 描述符(可变范围声明,非使用值)。
- 15/15 tsx 测试 + tsc --noEmit + vite build 全过。
- v8 截图(概览/词汇表/设置 × 浅/深)验收:层级清晰、无落档 YaHei 的西文、深浅两主题可读性正常。

### 2026-09-04 去叠层 pass 附带归一

- 清掉 `font:` 简写里的漏网非标值:650→600、750→700、800→700、12.5px→13px、13.5px→14px
  (此前 perl 扫描只覆盖 `font-size`/`font-weight` 独立属性,简写漏网)。
- 侧边栏导航 17px → 16px(归入 6 档字阶;52px 行高内容积不变)。
- 新增导航专用墨色 `--wi-nav-ink`(浅 0.78 / 深 0.85 alpha):侧栏薄纱上 `--wi-muted` 对比不足,
  导航/底栏文字切换过去;字重维持 500,靠颜色而非加粗保证可读性。
