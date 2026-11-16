import { mockBackend } from './mock-backend';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isTauri = (): boolean => {
  return typeof window !== 'undefined' && typeof window.__TAURI_INTERNALS__ !== 'undefined';
};

export async function invokeCommand<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<T>(cmd, args);
  }
  return mockBackend.invoke<T>(cmd, args);
}

export function listenEvent<T>(event: string, handler: (payload: T) => void): () => void {
  if (isTauri()) {
    let disposed = false;
    let unlistenFn: (() => void) | null = null;

    import('@tauri-apps/api/event')
      .then(({ listen }) => {
        if (disposed) return;
        return listen<T>(event, (e) => {
          if (!disposed) {
            handler(e.payload);
          }
        }).then((unlisten) => {
          if (disposed) {
            unlisten();
          } else {
            unlistenFn = unlisten;
          }
        });
      })
      .catch((err) => {
        console.warn(`[Tauri] native listen failed for "${event}":`, err);
      });

    return () => {
      disposed = true;
      if (unlistenFn) {
        unlistenFn();
        unlistenFn = null;
      }
    };
  }

  return mockBackend.listen<T>(event, handler);
}
