// 玻璃面板不透明度偏好：只缩放 glass.css 中「填充类」token 的 alpha
// （tint / sidebar / sheet / nav / pill 填充 / float / pop-surface）,
// rim 高光、描边、墨色不动 —— 调低时玻璃更通透,但边缘与文字对比不变。
// 持久化与主题同模式:localStorage；Tauri event 负责同步所有已加载窗口。

export const GLASS_ALPHA_STORAGE_KEY = 'ol.glass-alpha';
export const GLASS_ALPHA_EVENT = 'ui:glass-alpha-changed';
export const GLASS_ALPHA_MIN = 0.6;
export const GLASS_ALPHA_MAX = 1;
export const GLASS_ALPHA_DEFAULT = 1;

export function clampGlassAlpha(value: number): number {
  if (!Number.isFinite(value)) return GLASS_ALPHA_DEFAULT;
  return Math.min(GLASS_ALPHA_MAX, Math.max(GLASS_ALPHA_MIN, value));
}

export function readGlassAlpha(): number {
  try {
    if (typeof window === 'undefined') return GLASS_ALPHA_DEFAULT;
    const stored = window.localStorage.getItem(GLASS_ALPHA_STORAGE_KEY);
    if (stored === null) return GLASS_ALPHA_DEFAULT;
    return clampGlassAlpha(Number.parseFloat(stored));
  } catch {
    return GLASS_ALPHA_DEFAULT;
  }
}

export function applyGlassAlpha(alpha: number): void {
  try {
    if (typeof document === 'undefined') return;
    document.documentElement.style.setProperty('--lg-alpha-scale', String(clampGlassAlpha(alpha)));
  } catch {
    // Ignore restricted document access.
  }
}

export function setGlassAlpha(alpha: number): void {
  const clamped = clampGlassAlpha(alpha);
  applyGlassAlpha(clamped);
  try {
    if (typeof window === 'undefined') return;
    if (clamped === GLASS_ALPHA_DEFAULT) {
      window.localStorage.removeItem(GLASS_ALPHA_STORAGE_KEY);
    } else {
      window.localStorage.setItem(GLASS_ALPHA_STORAGE_KEY, String(clamped));
    }
  } catch {
    // Ignore restricted or quota-limited storage.
  }
  void broadcastGlassAlpha(clamped).catch(error => {
    console.warn('[glass-alpha] cross-window broadcast failed', error);
  });
}

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function broadcastGlassAlpha(alpha: number): Promise<void> {
  if (!hasTauriRuntime()) return;
  const { emit } = await import('@tauri-apps/api/event');
  await emit(GLASS_ALPHA_EVENT, clampGlassAlpha(alpha));
}

export async function startGlassAlphaSync(): Promise<() => void> {
  if (!hasTauriRuntime()) return () => undefined;
  const { listen } = await import('@tauri-apps/api/event');
  return listen<number>(GLASS_ALPHA_EVENT, event => {
    applyGlassAlpha(event.payload);
  });
}
