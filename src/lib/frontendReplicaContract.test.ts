// @ts-ignore This repo does not install Node type declarations, but the contract test runs under tsx.
import { readFileSync } from 'node:fs';

import {
  ABOUT_REQUIRED_COPY,
  ABOUT_REQUIRED_PRIMARY_ACTION,
  FORBIDDEN_ABOUT_LABELS,
  FORBIDDEN_HISTORY_ACTIONS,
  FORBIDDEN_RECORDING_LABELS,
  HISTORY_ROW_ACTION_LABELS,
  PREVIEW_NAV_ICON_PATHS,
  PREVIEW_VISUAL_TOKENS,
  REQUIRED_APP_TABS,
  REQUIRED_SETTINGS_SECTIONS,
  TOP_TOOL_LABELS,
} from './frontendReplicaContract';

function assert(condition: boolean, message: string) {
  if (!condition) throw new Error(message);
}

const settingsSource = readFileSync(new URL('../pages/Settings.tsx', import.meta.url), 'utf8');
const iconSource = readFileSync(new URL('../components/Icon.tsx', import.meta.url), 'utf8');
const sidebarSource = readFileSync(new URL('../components/shell/Sidebar.tsx', import.meta.url), 'utf8');
const commandsSource = readFileSync(new URL('../../src-tauri/src/commands.rs', import.meta.url), 'utf8');
const overviewSource = readFileSync(new URL('../pages/Overview.tsx', import.meta.url), 'utf8');
const windowChromeSource = readFileSync(new URL('../components/WindowChrome.tsx', import.meta.url), 'utf8');
const pageContainerSource = readFileSync(new URL('../components/shell/PageContainer.tsx', import.meta.url), 'utf8');
const previewCssSource = readFileSync(new URL('../styles/preview-replica.css', import.meta.url), 'utf8');
const ipcSource = readFileSync(new URL('../lib/ipc.ts', import.meta.url), 'utf8');

assert(
  REQUIRED_APP_TABS.join(',') === 'overview,history,vocab,style,settings',
  `required app tabs should match preview replica contract, got ${REQUIRED_APP_TABS.join(',')}`,
);

assert(
  REQUIRED_SETTINGS_SECTIONS.join(',') === 'models,recording,privacy,output,about',
  `required settings sections should match preview replica contract, got ${REQUIRED_SETTINGS_SECTIONS.join(',')}`,
);

assert(
  TOP_TOOL_LABELS.join(',') === '帮助,主题切换,中 / EN',
  `右上工具区必须保留帮助、主题切换、中 / EN，实际是 ${TOP_TOOL_LABELS.join(',')}`,
);

assert(
  HISTORY_ROW_ACTION_LABELS.join(',') === '复制,删除',
  `history row actions should only expose copy/delete, got ${HISTORY_ROW_ACTION_LABELS.join(',')}`,
);

assert(
  FORBIDDEN_HISTORY_ACTIONS.includes('重新润色'),
  '历史页必须禁止重新润色',
);

for (const label of FORBIDDEN_HISTORY_ACTIONS) {
  assert(
    !(HISTORY_ROW_ACTION_LABELS as readonly string[]).includes(label),
    `history row actions must not include forbidden action ${label}`,
  );
}

assert(
  FORBIDDEN_RECORDING_LABELS.includes('启用提示音'),
  'recording section must forbid the prompt sound label',
);

assert(
  FORBIDDEN_RECORDING_LABELS.includes('录音状态浮窗'),
  'recording section must forbid the recording status floating window label',
);

assert(
  FORBIDDEN_ABOUT_LABELS.includes('开发者'),
  'about section must forbid the developer label',
);

assert(
  FORBIDDEN_ABOUT_LABELS.includes('隐私政策'),
  'about section must forbid the privacy policy label',
);

const renderedRecordingLabels = ['全局快捷键', '录音模式', '麦克风设备', '输入音量测试'];
for (const forbidden of FORBIDDEN_RECORDING_LABELS) {
  assert(!renderedRecordingLabels.includes(forbidden), `录音设置不得出现 ${forbidden}`);
}

const renderedAboutLabels = ['官方网站', '指南', 'GitHub Star', '反馈渠道', ABOUT_REQUIRED_PRIMARY_ACTION];
for (const forbidden of FORBIDDEN_ABOUT_LABELS) {
  assert(!renderedAboutLabels.includes(forbidden), `关于页不得出现 ${forbidden}`);
}

const forbiddenSettingsSourceTokens = [
  'settings.recording.enableSound',
  'settings.recording.capsuleLabel',
  '启用提示音',
  '录音状态浮窗',
  'settings.about.developer',
  'settings.about.privacyPolicy',
];
for (const forbidden of forbiddenSettingsSourceTokens) {
  assert(!settingsSource.includes(forbidden), `Settings.tsx 不得重新引入 ${forbidden}`);
}

const requiredSettingsSourceTokens = [
  'settings.about.websiteLabel',
  'settings.about.docs',
  'settings.about.githubStarLabel',
  'settings.about.feedbackLabel',
  'https://github.com/EthanYoQ/whisper-input',
  '${repoUrl}#readme',
  '${repoUrl}/issues',
];
for (const required of requiredSettingsSourceTokens) {
  assert(settingsSource.includes(required), `Settings.tsx 必须包含 ${required}`);
}

assert(
  settingsSource.includes('SECTION_ICON_BY_ID') && settingsSource.includes('<Icon name={SECTION_ICON_BY_ID[s]}'),
  '设置二级 Tab 必须渲染对应图标，避免丢失原型图标层级',
);

assert(
  settingsSource.includes('bundleLogoSrc') && settingsSource.includes('wi-plan-logo'),
  '设置简单模式服务方案卡必须渲染千问/豆包品牌 Logo，不能只显示文字',
);

assert(
  settingsSource.includes('settings.providers.validateAsrSuccess') &&
    settingsSource.includes('settings.providers.validateLlmSuccess'),
  '设置页 ASR 检查必须显示配置完整语义，LLM 检查必须显示真实连接通过语义',
);

assert(
  settingsSource.includes('settings.advanced.streamingInsertTitle') &&
    settingsSource.includes('prefs.streamingInsert') &&
    settingsSource.includes('streamingInsert: next'),
  '设置页必须暴露“流式输出”开关，并真实绑定 prefs.streamingInsert',
);

assert(
  settingsSource.includes('wi-recording-settings-startup') &&
    settingsSource.includes('<AutostartRow />') &&
    settingsSource.includes('settings.recording.startupAtBoot'),
  '录音与热键页必须在可见启动卡片暴露开机自启，不得只藏在折叠分组里',
);

assert(
  settingsSource.includes('wi-recording-settings-left') &&
    settingsSource.includes('wi-recording-settings-right') &&
    previewCssSource.includes('.wi-recording-settings-left,') &&
    previewCssSource.includes('flex-direction: column;'),
  '录音与热键页必须使用左右两列独立堆叠，避免右侧启动卡片被左侧录音卡高度挤到下一行',
);

assert(
  ipcSource.includes('streamingInsert: true'),
  '前端 mock / 预览默认偏好必须开启流式输出，避免真实应用与预览体验不一致',
);

assert(
  ipcSource.includes("hotkey: { trigger: 'rightAlt'") &&
    ipcSource.includes("dictationHotkey: { primary: 'RightAlt', modifiers: [] }") &&
    ipcSource.includes("mode: 'hold'") &&
    ipcSource.includes('默认建议使用“按住右 Alt 说话”'),
  '前端 mock / 预览默认听写快捷键必须是 Right ALT 按住说话',
);

assert(
  settingsSource.includes('providerLogoSrc(') &&
    settingsSource.includes("providerLogoSrc(GEMINI_PROVIDER_ID)") &&
    !settingsSource.includes("logo: 'preview-qwen-logo.png'") &&
    !settingsSource.includes("logo: 'provider-qwen.svg'"),
  '设置页必须通过统一 providerLogoSrc 绑定千问、豆包、Gemini 官方图标，不能继续散落硬编码旧 PNG 或自绘 SVG',
);

assert(
  !settingsSource.includes('<ShortcutsSection />') && !settingsSource.includes('<PermissionsSection />'),
  '录音与热键页不得继续渲染“快捷键速查”和“权限”两个大卡片',
);

assert(
  previewCssSource.includes('width: min(720px, calc(100% - 330px))'),
  '设置二级 Tab 必须为右上工具区预留宽度，避免帮助/主题/语言按钮遮挡',
);

assert(
  previewCssSource.includes('height: 50px;') && previewCssSource.includes('min-height: 50px;'),
  '设置二级 Tab 必须固定 50px 高度，切换到录音与热键等长页面时不得被压扁',
);

assert(
  previewCssSource.includes('flex: 0 0 auto;') &&
    previewCssSource.includes('text-overflow: clip;') &&
    !previewCssSource.includes('.wi-settings-tab span {\n  min-width: 0;\n  overflow: hidden;\n  text-overflow: ellipsis;'),
  '设置二级 Tab 必须完整显示中文标签，不能用等宽压缩和省略号隐藏“录音与热键/隐私与数据/输出与语言”',
);

assert(
  previewCssSource.includes('padding-right: 280px;'),
  '页面头部必须为右上工具区预留安全区，避免风格页等页面操作区被遮挡',
);

assert(
  ABOUT_REQUIRED_COPY === '如果喜欢这个项目，请前往 GitHub 点亮 Star，支持继续迭代。',
  'about copy should ask users to star the project on GitHub',
);

assert(
  ABOUT_REQUIRED_PRIMARY_ACTION === '去 GitHub 点亮 Star',
  'about primary action should link users to star the project on GitHub',
);

for (const [iconName, paths] of Object.entries(PREVIEW_NAV_ICON_PATHS)) {
  for (const path of paths) {
    assert(
      iconSource.includes(path),
      `Icon.tsx 的 ${iconName} 导航图标必须复刻 preview.html path: ${path}`,
    );
  }
}

assert(
  sidebarSource.includes(`size={${PREVIEW_VISUAL_TOKENS.navIconSize}}`),
  `Sidebar 导航图标必须使用 ${PREVIEW_VISUAL_TOKENS.navIconSize}px`,
);

assert(
  !sidebarSource.includes('wi-brand'),
  'preview.html 的侧边栏不包含品牌块，Sidebar 不得保留额外品牌区',
);

assert(
  !sidebarSource.includes('本地服务运行中'),
  '标准 cloud-first 侧边栏不得显示“本地服务运行中”，避免把产品重新拉回本地 ASR 主线',
);

assert(
  overviewSource.includes(PREVIEW_VISUAL_TOKENS.modelLogoClass),
  '概览模型卡必须使用 preview.html 的模型 Logo 图片结构，不得退化为通用 stroke icon',
);

assert(
  !overviewSource.includes("Icon name={isAsr ? 'mic' : 'sparkle'}"),
  '概览模型卡不得使用 mic/sparkle 通用图标替代原型 Logo',
);

assert(
  overviewSource.includes('providerLogoSrc(asrProviderId)') &&
    overviewSource.includes('providerLogoSrc(llmProviderId)') &&
    !overviewSource.includes("preview-doubao-logo.png") &&
    !overviewSource.includes("provider-doubao.svg"),
  '概览页必须按当前 provider 使用统一官方图标映射，而不是硬编码旧 PNG 或自绘 SVG',
);

const historySource = readFileSync(new URL('../pages/History.tsx', import.meta.url), 'utf8');
assert(
  historySource.includes('DOUBAO_LLM_PROVIDER_ID') && historySource.includes('doubaoSeed20Lite'),
  '历史页必须识别豆包 LLM provider，并显示 Doubao-Seed-2.0-Lite，不能显示为 —',
);

assert(
  windowChromeSource.includes(`export const WIN_TITLEBAR_HEIGHT = ${PREVIEW_VISUAL_TOKENS.winTitlebarHeight}`),
  `Windows 标题栏高度必须复刻 preview.html 的 ${PREVIEW_VISUAL_TOKENS.winTitlebarHeight}px`,
);

assert(
  overviewSource.includes('className="wi-week-line-chart"') &&
    overviewSource.includes('<svg') &&
    !overviewSource.includes('wi-week-bar-track'),
  '近 7 天趋势必须使用折线图，不能继续使用会挤压底部文字的柱状图结构',
);

assert(
  commandsSource.includes('pub fn get_credentials(coord: CoordinatorState') &&
    commandsSource.includes('coord.prefs().get()') &&
    commandsSource.includes('credentials_status_from_snapshot('),
  '概览 provider 状态必须从 UserPreferences 的 active provider 派生，避免 CredentialsVault 与设置页分叉后显示旧 ASR/LLM',
);

assert(
  overviewSource.includes('maxChars') &&
    overviewSource.includes('point.day.chars') &&
    !overviewSource.includes('maxSessions'),
  '近 7 天趋势折线图纵轴必须按输入字数绘制，不能继续按段数绘制',
);

assert(
  windowChromeSource.includes('await currentWindow.hide();') &&
    !windowChromeSource.includes('await currentWindow.close();'),
  'Windows 自绘关闭按钮必须隐藏主窗口到托盘，不能直接 close 退出应用',
);

assert(
  pageContainerSource.includes("padding: 0"),
  'PageContainer 不得叠加额外 padding；页面内边距由 preview shell 控制',
);

const requiredCssTokens = [
  `padding: ${PREVIEW_VISUAL_TOKENS.mainPadding}`,
  `padding: ${PREVIEW_VISUAL_TOKENS.sidebarPadding}`,
  `height: ${PREVIEW_VISUAL_TOKENS.navButtonHeight}`,
  `font: ${PREVIEW_VISUAL_TOKENS.navFont}`,
  `top: ${PREVIEW_VISUAL_TOKENS.topToolsTop}`,
  `height: ${PREVIEW_VISUAL_TOKENS.topToolHeight}`,
  `grid-template-columns: ${PREVIEW_VISUAL_TOKENS.modelCardColumns}`,
];

for (const cssToken of requiredCssTokens) {
  assert(
    previewCssSource.includes(cssToken),
    `preview-replica.css 必须包含原型视觉令牌: ${cssToken}`,
  );
}
