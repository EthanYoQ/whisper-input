export type ThemePreference = 'light' | 'dark';

const THEME_STORAGE_KEY = 'ol.theme';
export const THEME_EVENT = 'ui:theme-changed';
const observers = new Set<(theme: ThemePreference) => void>();

export function subscribeThemePreference(observer: (theme: ThemePreference) => void): () => void {
  observers.add(observer);
  return () => { observers.delete(observer); };
}

function systemThemePreference(): ThemePreference {
  try {
    if (typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches) {
      return 'dark';
    }
  } catch {
    // Ignore restricted matchMedia access and use the stable default below.
  }
  return 'light';
}

export function readThemePreference(): ThemePreference {
  try {
    if (typeof window === 'undefined') return 'light';
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (stored === 'dark' || stored === 'light') return stored;
    return systemThemePreference();
  } catch {
    return 'light';
  }
}

export function applyThemePreference(theme: ThemePreference): void {
  try {
    if (typeof document === 'undefined') return;
    document.documentElement.dataset.theme = theme;
    observers.forEach(observer => observer(theme));
  } catch {
    // Ignore restricted document access.
  }
}

export function setThemePreference(theme: ThemePreference): void {
  applyThemePreference(theme);
  try {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Ignore restricted or quota-limited storage.
  }
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    void import('@tauri-apps/api/event').then(({ emit }) => emit(THEME_EVENT, theme))
      .catch(error => console.warn('[theme] broadcast failed', error));
  }
}

/** Register before re-reading storage so preloaded windows cannot miss a change. */
export async function startThemeSync(): Promise<() => void> {
  const onStorage = (event: StorageEvent) => {
    if (event.key === THEME_STORAGE_KEY || event.key === null) applyThemePreference(readThemePreference());
  };
  window.addEventListener('storage', onStorage);
  let unlisten = () => {};
  try {
    if ('__TAURI_INTERNALS__' in window) {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten = await listen<unknown>(THEME_EVENT, ({ payload }) => {
        if (payload === 'light' || payload === 'dark') applyThemePreference(payload);
      });
    }
    applyThemePreference(readThemePreference());
  } catch (error) {
    console.warn('[theme] listener failed', error);
  }
  return () => { window.removeEventListener('storage', onStorage); unlisten(); };
}
