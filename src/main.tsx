import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import i18n from "./i18n"; // 副作用：触发 i18next init
import { applyThemePreference, readThemePreference } from "./lib/themePreference";
import { applyGlassAlpha, readGlassAlpha, startGlassAlphaSync } from "./lib/glassAlpha";
import "./styles/tokens.css";
import "./styles/global.css";
import "./styles/preview-replica.css";
import "./styles/glass.css";

const params = new URLSearchParams(window.location.search);
const windowKind = params.get("window");
const isCapsule = windowKind === "capsule";
const isQa = windowKind === "qa";
const isSelectionPolish = windowKind === "selection-polish";

// 首帧前落实 data-theme:glass/replica 两套 token 都按它取色,
// 晚了会闪一帧错误主题。
applyThemePreference(readThemePreference());
// 同帧落实玻璃透明度:填充类 token 全部经 --lg-alpha-scale 取 alpha。
applyGlassAlpha(readGlassAlpha());
void startGlassAlphaSync().catch(error => {
  console.warn('[glass-alpha] cross-window listener failed', error);
});

// 纯浏览器 dev 预览:OS 合成器(Mica/Acrylic)不存在,窗口级玻璃
// token 按设计不含 backdrop-filter,透明 tint 落在浏览器默认白画布上
// 会读成「没有任何毛玻璃效果」。此处注入一层预模糊壁纸,模拟合成器
// 输出(与截图管线 shot-*.cjs 同配方),玻璃才有可透入的色相。
// Tauri 环境(webview 透明 + 真合成器)与生产构建一律不注入。
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  const wall = document.createElement("div");
  wall.id = "wi-preview-wallpaper";
  wall.style.cssText =
    "position:fixed;inset:-40px;z-index:-1;pointer-events:none;" +
    "background:" +
    "radial-gradient(50% 60% at 25% 25%, rgba(255,190,90,0.85), transparent 70%)," +
    "radial-gradient(60% 70% at 80% 20%, rgba(120,90,255,0.8), transparent 70%)," +
    "radial-gradient(70% 60% at 60% 90%, rgba(20,160,150,0.75), transparent 70%)," +
    "linear-gradient(135deg,#3b5bdb 0%,#7048e8 50%,#0b7285 100%);" +
    "filter:blur(48px) saturate(140%);";
  document.body.appendChild(wall);
}

const root = ReactDOM.createRoot(document.getElementById("root")!);

const renderApp = () => {
  root.render(
    <React.StrictMode>
      <App
        isCapsule={isCapsule}
        isQa={isQa}
        isSelectionPolish={isSelectionPolish}
      />
    </React.StrictMode>,
  );
};

// i18n 必须就绪后才能渲染：否则首次渲染拿到的 t() 返回 key 字面量。
// react-i18next useSuspense=false 时不会自动等，只有事件触发后重渲染才能拿到译文。
if (i18n.isInitialized) {
  renderApp();
} else {
  i18n.on("initialized", renderApp);
}
