export type NativeMaterial = 'acrylic' | 'mica' | 'vibrancy' | 'fallback';

export function applyNativeMaterial(material: unknown): void {
  if (material === 'acrylic' || material === 'mica' || material === 'vibrancy' || material === 'fallback') {
    document.documentElement.dataset.nativeMaterial = material;
  }
}

export async function initializeNativeMaterial(): Promise<void> {
  if (!('__TAURI_INTERNALS__' in window)) return;
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    applyNativeMaterial(await invoke<NativeMaterial>('get_native_material_status'));
  } catch (error) {
    // A missing/late IPC response is NOT evidence that the OS material failed.
    console.warn('[material] status unavailable', error);
  }
}
