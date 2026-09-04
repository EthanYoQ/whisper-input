/** Dispose late subscriptions too (React StrictMode/unmount and HMR). */
export function asyncSubscription(
  register: () => Promise<() => void>,
  onError: (error: unknown) => void,
): () => void {
  let disposed = false;
  let unsubscribe: (() => void) | undefined;
  void Promise.resolve().then(register).then(stop => {
    if (disposed) stop();
    else unsubscribe = stop;
  }).catch(error => { if (!disposed) onError(error); });
  return () => {
    if (disposed) return;
    disposed = true;
    unsubscribe?.();
  };
}
