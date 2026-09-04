# design-system.md — Whisper Input 设计系统决策台账

> 依据 `.claude/skills/ui-design`(charlomrt-boop/ui-design-skill)MIGRATE 模式要求建立。
> MIGRATE = 提取现存主导 pattern → 选边统一 → 渐进迁移;不推翻重来。
> 排版部分详见 [TYPOGRAPHY.md](TYPOGRAPHY.md),玻璃材质部分见 `src/styles/glass.css` 头注。

## 模式定位

- **模式**:MIGRATE(存量迁移)
- **主导 pattern(迁移前审计)**:毛玻璃 token 体系(glass.css `--lg-*`)+ 手写内联样式混杂
- **选边**:所有尺寸值向 token 体系收敛;字面量只作为 token 的实现细节存在,不直接出现在组件层

## 圆角体系(同心递减,ui-design §4.4 反 AI-slop:组件半径必须成体系)

| Token | 值 | 层级 | 用途 |
|---|---|---|---|
| `--ol-r-xs` | 6px | 徽章 | 小徽章、计数点 |
| `--ol-r-sm` | 8px | 控件 | 按钮、输入框、开关、下拉 |
| `--ol-r-md` | 12px | 卡片 | 内容卡片、模式选择卡 |
| `--ol-r-lg` | 16px | 大面 | 正文 sheet、模态框、浮窗 |
| `--ol-r-pill` | 999px | 胶囊 | 录音胶囊、药丸按钮、圆形头像钮 |

规则:嵌套时内层永远比外层小一档(大面 16 → 卡 12 → 控件 8 → 徽章 6),符合同心圆几何。

**迁移结果**:18 个字面量半径 → {6, 8, 12, 16, 999}(另有 3px 小圆点、50% 正圆属合法几何值)。
修复了旧 token(`--ol-r-sm/md/lg/xl/2xl`)零引用、组件层全写字面量的失效状态——新 token 即按实际使用量定档。

## 字阶与字重

见 [TYPOGRAPHY.md](TYPOGRAPHY.md)。要点:字阶 6 档(§6.3 上限 7–8 档内)、字重 4 档、数字列 tabular-nums(§6.6)。

## 色彩与材质

- **玻璃体系**:全部表面色定义于 `src/styles/glass.css` 的 `--lg-*` token,浅/深双主题成对定义。
- **唯一定义点**:`--ol-card-bg` / `--wi-panel` 只在 glass.css 定义一次(Vite HMR 会把后加载的 style 重插到 head 末尾,同名变量双定义会导致旧值复活)。
- **反 AI-slop §4.1**:禁纯 #000 纯 #fff 直接铺面——深色面用 #16181f/#1c1e26 系,浅色面用带蓝相的白(253 254 255 / 244 247 252)。
- **透明度滑杆**:填充类 alpha 全经 `calc(α × var(--lg-alpha-scale))`,rim/描边/墨色不缩放(见 `src/lib/glassAlpha.ts`)。

## 间距

沿用 4px 基网,常用档位 4 / 8 / 12 / 16 / 24(组件内 padding 与 gap)。本轮未做归一(审计未发现失控碎档),后续若改布局须遵守。

## Session Log

### 2026-09-04 美化 pass

1. **Phase 1(变量集中化)**:tokens.css 重写头部——新圆角 token、字阶 token、字重 token、字体栈;删除 Google Fonts 外链。
2. **全量归一(perl 批量)**:4 个 CSS 文件 + 全部 TSX 内联样式按映射表归一(字号/字重/圆角)。
3. **手工微调**:sheet 圆角 12→16(与浮窗同档,大面读感);_atoms h1 22/700/1.25;SettingsModal h2 对齐页面标题档;Card.tsx radius 18→12;词汇表卡 780px 限宽居中。
4. **验收**:v8 六图(概览/词汇表/设置 × 浅/深)+ 15/15 测试 + tsc + build。

### 2026-09-04 去叠层 pass(用户反馈:嵌套板块越叠越不透明 + 导航文字太浅)

**根因(用户猜测证实)**:浅色主题的控件层 token 仍是不透明遗留值
(`--wi-control: #fff`、`--wi-control-muted: #f3f6fa`、`--ol-surface-2: #f8f8f9`、
`.wi-btn` 硬编码 `#fff`),每嵌套一层就叠一次不透明填色,壁纸色相被彻底挡死。
深色主题此前已玻璃化(白烟 0.06/0.08),所以问题集中在浅色。

**根本修复(liquid-glass skill 规则 3:玻璃面板的子元素不再涂自己的表面)**:

| Token | 浅色改前 | 浅色改后 | 深色 |
|---|---|---|---|
| `--wi-control`(按钮/输入) | `#fff`(不透明) | 白纱 0.45×α | 白烟 0.06(原样) |
| `--wi-control-muted`(分组容器) | `#f3f6fa`(不透明) | **墨烟 0.045×α**(与深色白烟镜像) | 白烟 0.08(原样) |
| `--wi-control-active`(选中面) | 不透明白渐变 | 白纱 0.78×α | 蓝烟 0.16(原样) |
| `--ol-surface-2`(下拉/输入/小按钮) | `#f8f8f9` / 深 `#2b2c31` | 白纱 0.45×α / 白烟 0.08×α | 同左 |
| `--lg-card-bg`(卡片) | 0.24/0.20 | **0.14/0.10**(叠 sheet 只抬半档) | 0.055/0.04(原样) |
| `--wi-panel` | 0.26 | 0.16 | 0.05(原样) |
| `*-soft` 徽章底色(蓝/紫/绿/橙) | 不透明 `#eaf3ff` 等 | 彩色纱 0.10–0.12×α | 已半透明(原样) |

**配套重定向**:`.wi-btn` 基座 `#fff` → `var(--wi-control)`;顶栏按钮 0.82 白 → token;
`miniBtnStyle`/`iconBtnStyle`/分段控件激活态/LocalAsr 下拉/LocalSpeech 输入的
`var(--ol-surface)` 与 `'#fff'` → `var(--ol-surface-2)`;服务商卡片/周趋势图/图表注释
的不透明白底 → `--wi-panel` / 白纱渐变 / `--wi-control-muted`。
`--ol-surface` 保持不透明:它是模态/浮窗级霜面(modal tier,skill 允许),不做控件用。

**导航可读性**:新增 `--wi-nav-ink`(浅 `rgba(15,23,42,0.78)` / 深 `rgba(245,247,251,0.85)`),
侧边栏导航与底栏版本号从 `--wi-muted` 切换到该 token —— 侧栏薄纱比 sheet 透得多,
通用 muted 色在其上对比不足。

**验证**:v9 六图(隐私/关于/输出语言/概览 × 浅/深)+ 15/15 测试 + tsc + build。
隐私页三组红框容器、关于页按钮列、输出语言页下拉均恢复玻璃通透感;导航文字深浅两色清晰可读。

### 2026-09-04 侧边栏圆角/宽窄 pass(make-interfaces-feel-better skill)

**Skill 装载**:`jakubkrehel/make-interfaces-feel-better` 安装为项目级 skill
(`.claude/skills/make-interfaces-feel-better`),本轮应用其 §1 同心圆角原则
(outerRadius = innerRadius + padding;相邻面之间的缝给圆角留呼吸)。

**圆角问题根因**:sheet(`.wi-main`)margin 为 `0 8px 8px 0`——上缘贴死标题栏、左缘贴死
侧边栏列,16px 圆角在窗口顶缘与栏缘处被「裁切」;同时侧边栏薄纱(0.26 tint)与
sheet(0.60 tint)边贴边,两种不同浓度 tint 形成硬接缝。

**修复**:sheet 四周浮起 `margin: 8px`(Arc 式浮板)——四角 16px 弧线完整可读,
栏与 sheet 之间的硬接缝变为 8px 均匀玻璃 gutter,外壳 Acrylic/Mica 四边均匀露出。

**宽窄决策(固定 216px,弃 vw)**:
- 内容积测算:最长导航标签「词汇表」(3×16px CJK)+ 图标 18 + gap 16 + 内边距 22/18 ≈ 122px。
- `clamp(205px, 20vw, 250px)` → 固定 `216px`:余量克制(内宽 196px),不随窗口宽度生长,
  宽屏多出的空间全部让给正文。
- 1240px 窗口实测比例:侧栏 216 : sheet 外宽 1008 ≈ **17.6% : 82.4%**(内容优先)。
- ≤1100px 媒体查询保留 205px 窄窗档,未动。

**契约同步**:`PREVIEW_VISUAL_TOKENS.navFont` `500 17px/1` → `500 16px/1`
(补上轮 typography 归一后的陈旧断言,17px 破字阶改为 16px 是既定决策,见 TYPOGRAPHY.md)。

**验证**:预览实测几何(侧栏 216px、sheet x=224 y=64、四缝均 8px、radius 16px)+
v10 四图(概览/设置 × 浅/深)+ 15/15 tsx 测试 + tsc --noEmit + vite build 全绿。

### 2026-09-04 三连微调 pass(悬浮式侧栏 + 顶栏对齐 + 蓝色克制)

接用户三张标注截图的三个问题,全部按 make-interfaces-feel-better 与 liquid-glass 原则落地。

**问题 1——侧栏上角缺角/与 sheet 顶不齐**:
根因:侧栏整列贴死(顶缘=标题栏底、左/底贴窗口缘、直角),sheet 浮起 8px,
栏顶比 sheet 高 8px,.sheet 左上圆角旁杵着侧栏的直角。
修复:侧栏改为与 sheet 同规格的浮板——`margin: 8px 0 8px 8px`、`border-radius: 16px`
(同 `--lg-sheet-radius`)、`inset ring + --lg-sheet-shadow`。实测两块浮板
y=64、h=728 完全同高,四角弧线完整(Arc 现行双浮板形态)。侧栏有效宽 208px。

**问题 2——设置 Tab 条与帮助/语言按钮错位**:
根因:Tab 条 50px 高、顶缘在 sheet padding 31 处;顶栏按钮 38px 高、top:27——
高度与顶缘双重错位。
修复(全页面统一规则):`.wi-top-tools` top 27→**31**(= sheet padding-top),
`.wi-settings-tabs` 50→**38px**(= 顶栏按钮高),窄窗媒体查询同步 20→24。
实测 Tab 条与按钮组同为 y=95、h=38,同一水平带。「帮助/语言切换的高度」
由此在每个页面都是 38px、顶缘与正文内容顶缘齐平。
契约同步:`topToolsTop` 27→31;Tab 防压扁断言 50px→38px(min-height + flex:0 0 auto 的
反挤压不变量保留)。

**问题 3——实心蓝块突兀(liquid-glass 规则 2:大面不铺纯色,颜色由纱层透出)**:
- `.wi-model-mode button.active`(简单模式段选)与 `.wi-mode-option-active`:
  实心 `var(--wi-blue)` → **蓝纱 `--wi-blue-soft` 叠导航白纱 `--lg-nav-active`**,
  白字 → 蓝字 `--wi-blue`,描述小字回归 `--wi-muted`;+ nav-active ring/投影。
  与设置 Tab、侧边导航的激活态统一为同一套语言,深浅两色自动成立
  (深色为蓝纱 0.18 叠白烟 0.10)。
- `.wi-btn-primary`: `#2563eb` 与 `--wi-blue` #0f6fff **两种蓝并存** → 统一为
  #0f6fff 色相的微渐变纱化蓝(0.92→0.82)+ rim 高光;主行动按钮保留品牌强调,
  但不再是死实心块;不随透明度滑杆缩放(主行动点需稳定可辨)。
- `.wi-style-pack-current`(风格页「当前」徽章):实心蓝 → `--wi-blue-soft` + 蓝字,
  与 `.wi-pill-blue` 完全同语言。
- 保留实心蓝的小尺寸功能点:单选圆点、开关、计数徽章(9–20px 级,iOS 惯例,
  读作强调点而非表面)。

**验证**:预览实测(两浮板同高 728、Tab/按钮同带 38px@y95、段选激活态
bg=蓝纱+白纱、字色 rgb(15,111,255),深浅双色)+ v11 四图 + 15/15 测试 +
tsc + build 全绿。

### 2026-09-04 合并后玻璃审查修复

- QA 与划词润色浮窗改用成对的 `--lg-float-*` 主题 token，清除未定义的
  `--ol-ink-1`、`--ol-surface-1`、`--ol-danger`。
- Windows 主窗口优先 Mica，Windows 10 才回退 Acrylic；只有原生调用成功时
  才标记 `data-native-glass=on`，否则主窗口使用实体底色。删除 Windows 8 起
  不再产生模糊的胶囊 `DwmEnableBlurBehindWindow` 路径。
- 透明度通过 `ui:glass-alpha-changed` Tauri event 同步四个 webview；预览控件与
  普通 `--ol-surface-2` 控件均由 45% 降为 18%，激活层由 78% 降为 20%，
  导航激活层由 68% 降为 18%。
- 新增减少透明度、高对比度降级；直接依赖对齐 Tauri 的
  `window-vibrancy 0.6`，避免 macOS 同时链接 0.6/0.7；补齐 Inter OFL 1.1。

### 2026-09-04 人工验收修正

- Windows 实机验收确认 Mica 虽成功启用，但深色模式下过于均匀，叠加烟熏
  sheet 后失去 KIMI `d724b2e` 的明显毛玻璃质感。恢复 Acrylic 优先、Mica
  降级；继续保留原生失败实体底色与无障碍降级。
- 主窗口命令栏在语言切换旁新增 38px Light/Dark 图标按钮，复用既有
  `sun` / `moon` 图标、`ol.theme` 持久化与五语言辅助文案。

### 2026-09-04 KIMI 毛玻璃基线恢复（取代上两节的实体降级决策）

- 以 KIMI 完成态 `d724b2e` 作为材质权威基线，不重新设计透明度、面板层次或
  色彩语言；本轮只修合并后审查引入的回归，并保留新增的 Light/Dark 切换。
- Windows 视觉回归的根因是直接依赖从 KIMI 使用的 `window-vibrancy 0.7`
  降至 0.6，同时加入 `data-native-glass` 实体底色门控、系统“减少透明度”
  实体覆盖，并把多组 KIMI 玻璃纱层大幅削薄。DWM 返回成功不等于真实画面
  已与基线一致，今后不得只用 API 状态替代截图对照。
- 恢复 KIMI 材质强度：浅色导航激活层 0.68、普通控件层 0.45、激活控件层
  0.78、`--ol-surface-2` 0.45；移除主窗口原生状态门控和减少透明度实体覆盖，
  保留 Windows 强制色彩模式的可读性降级。
- 依赖按平台拆分：Windows 使用 `window-vibrancy 0.7`，继续 Acrylic 优先、
  Mica 降级；macOS 维持与 Tauri 对齐的 0.6，避免重复符号链接问题。
- 同一彩色条纹背景实机截图的平均 RGB 色差：KIMI 80.63、损坏版 6.32、
  修复版 81.93。修复版恢复整窗颜色穿透与模糊，视觉强度与 KIMI 基线一致。

### 2026-09-04 Windows 外围白框修正

- 实机像素检查确认四周约 7–8px 的亮框来自 Windows `WS_THICKFRAME`
  非客户区，不是 React 面板边框。单纯移除该 style 虽能去框，却会偏离 KIMI
  `d724b2e` 的 Acrylic 与原生缩放基线，因此该方案作废。
- 保留 KIMI 的 `WS_THICKFRAME`、Acrylic 优先/Mica 降级及前端
  `startResizeDragging` 原路径；根据 `GetWindowRect`、`GetClientRect` 和
  `ClientToScreen` 实测非客户区厚度，只把该区域裁出可见窗口 region。
- 未调整任何 `glass.css` 材质 token、透明度默认值或 Light/Dark 配方。
  实机复核为 `WS_THICKFRAME` 仍在、外框不可见、原生横向缩放可增加 80px。
- 最小化时 Windows 会临时报告约 `160x28` 的任务栏图标矩形；此时禁止重写
  window region，并在恢复焦点后立即及延迟 150ms 各重套一次客户区裁剪，等待
  异步恢复的正常窗口矩形稳定后覆盖系统图标 region，避免原生标题栏重新露出。
- 重新唤起隐藏且最小化的主窗口时，必须先 `unminimize` 再 `show`；反向顺序会
  把已显示窗口留在 `(-32000,-32000)` 的 iconic placement，裁剪继续停在 160x28。
- 主窗口失去焦点时隐藏，符合托盘型输入法“点击桌面即收起”的交互；再次从
  托盘、热键或单实例入口显示时，会先恢复窗口再重套裁剪。Windows 使用
  `EVENT_SYSTEM_FOREGROUND` 监听真实前台进程变化：切到桌面或其他应用时直接
  隐藏主 HWND；外部切换需持续 75ms 才执行，以避开唤起过程中的瞬时焦点交接，
  同进程的胶囊、QA 与选区润色窗口不会误触发。

### 2026-09-04 Light 概览叠层降噪

- 实机 Light 截图确认透明度问题来自嵌套叠层，不是原生 Acrylic 失效：侧栏选中项
  在侧栏薄纱上再叠 68% 白纱；概览分组在正文 sheet 上再叠卡片薄纱，并以不透明
  `#edf3f8` 画 1px 外框与内部隔线。
- 保留 KIMI 的导航布局、蓝色状态语言和 Overview 分组结构，只将 Light 导航选中层
  改为 28% 白→16% 灰的无色渐变薄纱、hover 降为 10%，同时弱化 ring 与投影。
  Dark 的 10% 白烟配方不变。
- Overview 的模型组、指标组、趋势与最近识别面板不再绘制第二层 tint；仅保留
  `0.5px rgba(30, 33, 45, 0.065)` 暗色 hairline。Dark 继续使用既有
  `--lg-card-bg` 与 7% 白色边线，未改动其毛玻璃层次。

### 2026-09-05 Light 单层玻璃收敛

- 依据项目 `.claude/skills/liquid-glass` 的规则 3，Light 模式采用“一条视觉栈只允许
  一张玻璃面”：侧栏和正文 sheet 是玻璃父层，内部导航项与 Overview 内容块不再
  绘制自己的背景、边框、rim 或第二层圆角。
- Light 侧栏薄纱从 26%/16% 降为 14%/8%，正文 sheet 从 60%/52% 降为
  32%/24%；正文仍由 Windows Acrylic 提供真实桌面采样和模糊，不增加 CSS 假背景。
- 侧栏选中态移除白色 chip、ring 和阴影，改由蓝色图标/文字及 600 字重表达状态；
  hover 仅保留 6% 瞬态白纱。
- Overview 的模型、指标、趋势和最近识别区域在 Light 下全部透明，以间距和排版分组；
  Dark 仍显式恢复原有 5% 白烟薄纱、0.5px rim 与 12px 圆角，避免改变已通过的深色材质。

### 2026-09-05 KIMI 父玻璃与窗口生命周期最终校正

- 上一节继续削薄侧栏与正文父层会让 Acrylic 失去 KIMI `d724b2e` 的材质读感，已撤销：
  Light 侧栏恢复 26%/16%，正文 sheet 恢复 60%/52%。单层规则只作用于嵌套内容，
  Overview 子分组仍保持透明、无边框、无第二层圆角；导航选中项使用 28%→16% 薄纱。
- 独立审计确认 `EVENT_SYSTEM_FOREGROUND` 全局钩子会把托盘或单实例的显式恢复与
  自动隐藏互相竞争，导致“程序仍在但主窗口和任务栏入口无法恢复”。该钩子已删除，
  改由 main 窗口自身的 `Focused(false)` 事件在 Tauri 主线程判定并隐藏；显式恢复有
  750ms 焦点交接保护，且统一先 `unminimize`、再 `show`、最后 `set_focus`。
- release 实机门禁连续 10 次通过“外部真实点击 → 主窗口隐藏且进程存活 → 二次启动恢复
  到前台 → 保护期结束仍稳定”，隐藏耗时 132–160ms，恢复耗时 139–161ms。
- Light/Dark 均在真实 Tauri WebView 中验证；Light 1240px 与 1024px 下 Overview 的
  模型、指标、趋势、最近识别均无嵌套表面。受控彩色背景截图中没有原生标题栏或四周
  白框，Win32 探针同时确认 `WS_CAPTION=false`、`WS_EX_APPWINDOW=true`。

### 2026-09-05 失焦行为校正：保留任务栏入口

- 人工反馈进一步明确：点击应用外部后主界面应退出桌面，但任务栏入口必须保留，不能
  像托盘关闭那样完整 `hide`。因此上一节所述的失焦“隐藏”状态机改为 Windows 原生
  `minimize`；窗口仍具备 `WS_EX_APPWINDOW`，可从任务栏、托盘或二次启动恢复。
- 标题栏关闭按钮仍沿用 hide-to-tray，失焦与关闭现在是两个明确且互不混淆的状态。
