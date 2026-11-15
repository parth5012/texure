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
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<T>(cmd, args);
    } catch (err) {
      console.warn(`[Tauri] native invoke failed for "${cmd}", falling back to mock backend:`, err);
      return mockBackend.invoke<T>(cmd, args);
    }
  }
  return mockBackend.invoke<T>(cmd, args);
}

export function listenEvent<T>(event: string, handler: (payload: T) => void): () => void {
  if (isTauri()) {
    let unlistenFn: (() => void) | null = null;
    import('@tauri-apps/api/event').then(({ listen }) => {
      listen<T>(event, (e) => handler(e.payload)).then((unlisten) => {
        unlistenFn = unlisten;
      });
    }).catch((err) => {
      console.warn(`[Tauri] native listen failed for "${event}", falling back to mock:`, err);
      unlistenFn = mockBackend.listen<T>(event, handler);
    });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }

  return mockBackend.listen<T>(event, handler);
}
