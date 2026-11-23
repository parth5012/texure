import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useCompanion } from '../context/CompanionContext';
import {
  Sun,
  Unlock,
  Play,
  Square,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  Info,
  X,
  Sliders,
  Smartphone,
  Radio,
  Layers,
} from 'lucide-react';

interface Toast {
  id: string;
  type: 'success' | 'error' | 'info';
  title: string;
  message: string;
}

// Configurable default target app package
const DEFAULT_PACKAGE_NAME = (import.meta as { env?: Record<string, string> }).env?.VITE_DEFAULT_TARGET_PACKAGE || 'com.example.target';

// 4-second display duration provides ample reading time for status messages without obstructing viewport permanently
const TOAST_DURATION_MS = 4000;

// Sanitize string content for safe display
const sanitizeToastText = (raw: string): string => {
  return raw.replace(/<[^>]*>?/gm, '').slice(0, 300);
};

// Pure styling helper for toast notifications
const getToastStyles = (type: 'success' | 'error' | 'info'): string => {
  switch (type) {
    case 'success':
      return 'bg-slate-900 border-emerald-500/40 text-emerald-300';
    case 'error':
      return 'bg-slate-900 border-rose-500/40 text-rose-300';
    case 'info':
    default:
      return 'bg-slate-900 border-indigo-500/40 text-indigo-300';
  }
};

export const ControlPanel: React.FC = () => {
  const {
    state,
    wakeScreen,
    dismissKeyguard,
    coldStartApp,
    forceStopApp,
    reconnectTunnel,
  } = useCompanion();

  const [packageName, setPackageName] = useState<string>(DEFAULT_PACKAGE_NAME);
  const [loadingAction, setLoadingAction] = useState<string | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastTimeoutsRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  // Clean up all toast timeouts on unmount
  useEffect(() => {
    return () => {
      toastTimeoutsRef.current.forEach((timeout) => clearTimeout(timeout));
      toastTimeoutsRef.current.clear();
    };
  }, []);

  const dismissToast = useCallback((id: string) => {
    const existing = toastTimeoutsRef.current.get(id);
    if (existing) {
      clearTimeout(existing);
      toastTimeoutsRef.current.delete(id);
    }
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const addToast = useCallback((type: 'success' | 'error' | 'info', title: string, message: string) => {
    const id = `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
    const sanitizedTitle = sanitizeToastText(title);
    const sanitizedMsg = sanitizeToastText(message);
    const newToast: Toast = { id, type, title: sanitizedTitle, message: sanitizedMsg };

    setToasts((prev) => [...prev, newToast]);

    const timeout = setTimeout(() => {
      dismissToast(id);
    }, TOAST_DURATION_MS);

    toastTimeoutsRef.current.set(id, timeout);
  }, [dismissToast]);

  const handleAction = async (actionId: string, fn: () => Promise<unknown>, successMsg: string) => {
    if (loadingAction) return;
    setLoadingAction(actionId);

    try {
      await fn();
      addToast('success', 'Action Executed', successMsg);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      addToast('error', 'Action Failed', errorMsg);
    } finally {
      setLoadingAction(null);
    }
  };

  const isExecuting = loadingAction !== null;
  const effectivePackage = (packageName || DEFAULT_PACKAGE_NAME).trim();

  return (
    <div data-testid="control-panel" className="space-y-6">
      {/* Toast Overlay */}
      <aside aria-label="Notifications" className="fixed top-5 right-5 z-50 flex flex-col space-y-2 max-w-sm w-full pointer-events-none">
        {toasts.map((toast) => (
          <div
            key={toast.id}
            data-testid="toast-notification"
            className={`pointer-events-auto p-4 rounded-xl border shadow-xl flex items-start space-x-3 transition-all transform animate-in slide-in-from-top duration-200 ${getToastStyles(
              toast.type
            )}`}
          >
            {toast.type === 'success' && <CheckCircle2 className="w-5 h-5 text-emerald-400 shrink-0 mt-0.5" />}
            {toast.type === 'error' && <AlertCircle className="w-5 h-5 text-rose-400 shrink-0 mt-0.5" />}
            {toast.type === 'info' && <Info className="w-5 h-5 text-indigo-400 shrink-0 mt-0.5" />}
            <div className="flex-1 min-w-0">
              <h4 className="text-xs font-semibold text-slate-100">{toast.title}</h4>
              <p data-testid="toast-message" className="text-xs text-slate-300 mt-0.5 break-words">
                {toast.message}
              </p>
            </div>
            <button
              onClick={() => dismissToast(toast.id)}
              data-testid="toast-dismiss"
              className="text-slate-500 hover:text-slate-300 transition shrink-0 p-0.5 rounded"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        ))}
      </aside>

      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold tracking-tight text-slate-100 flex items-center space-x-2">
            <Sliders className="w-5 h-5 text-indigo-400" />
            <span>Device Manual Control Panel</span>
          </h2>
          <p className="text-xs text-slate-400 mt-1">
            Direct hardware & lifecycle override commands executed via Shizuku shell IPC and WebSocket bridge.
          </p>
        </div>

        {/* Global indicator */}
        <div className="flex items-center space-x-2">
          <span
            className={`w-2 h-2 rounded-full ${
              isExecuting ? 'bg-amber-400 animate-ping' : 'bg-emerald-400'
            }`}
          />
          <span className="text-xs font-mono text-slate-400">
            {isExecuting ? 'COMMAND RUNNING' : 'IDLE / READY'}
          </span>
        </div>
      </div>

      {/* Control Action Cards Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Section 1: Screen & Display Controls */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center space-x-2 text-indigo-400 mb-3">
              <Smartphone className="w-4 h-4" />
              <h3 className="text-sm font-semibold text-slate-200">Screen & Keyguard</h3>
            </div>
            <p className="text-xs text-slate-400 mb-5">
              Power key dispatch and lockscreen dismissal using Shizuku privileged shell access.
            </p>

            <div className="space-y-3">
              {/* Wake Screen Button */}
              <button
                type="button"
                onClick={() =>
                  handleAction(
                    'wake_screen',
                    wakeScreen,
                    'Wake screen command (input keyevent 26) dispatched successfully'
                  )
                }
                disabled={isExecuting}
                data-testid="btn-wake-screen"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-800 disabled:text-slate-500 text-white text-xs font-medium transition shadow-md"
              >
                <span className="flex items-center space-x-2">
                  {loadingAction === 'wake_screen' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-indigo-300" />
                  ) : (
                    <Sun className="w-4 h-4" />
                  )}
                  <span>Wake Screen</span>
                </span>
                <span className="font-mono text-[10px] opacity-75">KEYEVENT 26</span>
              </button>

              {/* Dismiss Keyguard Button */}
              <button
                type="button"
                onClick={() =>
                  handleAction(
                    'dismiss_keyguard',
                    dismissKeyguard,
                    'wm dismiss-keyguard dispatched successfully'
                  )
                }
                disabled={isExecuting}
                data-testid="btn-dismiss-keyguard"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg bg-slate-800 hover:bg-slate-700 disabled:bg-slate-800/50 disabled:text-slate-600 text-slate-200 text-xs font-medium transition border border-slate-700"
              >
                <span className="flex items-center space-x-2">
                  {loadingAction === 'dismiss_keyguard' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-indigo-400" />
                  ) : (
                    <Unlock className="w-4 h-4 text-emerald-400" />
                  )}
                  <span>Dismiss Keyguard</span>
                </span>
                <span className="font-mono text-[10px] text-slate-400">wm unlock</span>
              </button>
            </div>
          </div>

          <div className="mt-5 pt-3 border-t border-slate-800 text-[11px] text-slate-500">
            Current Screen: <strong className="text-slate-300">{state.vitals.screenOn ? 'ON' : 'OFF'}</strong>
          </div>
        </div>

        {/* Section 2: Application Lifecycle Controls */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center space-x-2 text-indigo-400 mb-3">
              <Layers className="w-4 h-4" />
              <h3 className="text-sm font-semibold text-slate-200">Application Lifecycle</h3>
            </div>
            <p className="text-xs text-slate-400 mb-3">
              Trigger cold launch or force-stop for the target application package.
            </p>

            {/* Target Package Input */}
            <div className="mb-4">
              <label htmlFor="target-package-input" className="block text-[11px] font-medium text-slate-400 mb-1">
                Target Package
              </label>
              <input
                id="target-package-input"
                type="text"
                value={packageName}
                onChange={(e) => setPackageName(e.target.value.trim())}
                data-testid="input-package-name"
                className="w-full px-3 py-1.5 bg-slate-950 border border-slate-800 rounded-lg text-xs font-mono text-slate-200 focus:outline-none focus:border-indigo-500 transition"
                placeholder="com.example.target"
              />
            </div>

            <div className="space-y-3">
              {/* Cold Start Button */}
              <button
                type="button"
                onClick={() =>
                  handleAction(
                    'cold_start',
                    () => coldStartApp(effectivePackage),
                    `Target app ${effectivePackage} launched via am start`
                  )
                }
                disabled={isExecuting}
                data-testid="btn-cold-start"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 disabled:bg-slate-800 disabled:text-slate-500 text-white text-xs font-medium transition shadow-md"
              >
                <span className="flex items-center space-x-2">
                  {loadingAction === 'cold_start' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-emerald-300" />
                  ) : (
                    <Play className="w-4 h-4 fill-white" />
                  )}
                  <span>Cold Start App</span>
                </span>
                <span className="font-mono text-[10px] opacity-75">am start</span>
              </button>

              {/* Force Stop Button */}
              <button
                type="button"
                onClick={() =>
                  handleAction(
                    'force_stop',
                    () => forceStopApp(effectivePackage),
                    `Target app ${effectivePackage} killed via am force-stop`
                  )
                }
                disabled={isExecuting}
                data-testid="btn-force-stop"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg bg-rose-600/20 hover:bg-rose-600/30 disabled:bg-slate-800/50 disabled:text-slate-600 text-rose-300 text-xs font-medium transition border border-rose-500/30"
              >
                <span className="flex items-center space-x-2">
                  {loadingAction === 'force_stop' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-rose-400" />
                  ) : (
                    <Square className="w-4 h-4 fill-rose-400" />
                  )}
                  <span>Force Stop App</span>
                </span>
                <span className="font-mono text-[10px] text-rose-400">am force-stop</span>
              </button>
            </div>
          </div>

          <div className="mt-5 pt-3 border-t border-slate-800 text-[11px] text-slate-500">
            Process Status: <strong className="text-slate-300">{state.appStatus}</strong>
          </div>
        </div>

        {/* Section 3: Connection & WebSocket Tunnel */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center space-x-2 text-indigo-400 mb-3">
              <Radio className="w-4 h-4" />
              <h3 className="text-sm font-semibold text-slate-200">Reverse WS Tunnel</h3>
            </div>
            <p className="text-xs text-slate-400 mb-5">
              Reset and re-establish the encrypted WebSocket tunnel connection between phone and desktop bridge.
            </p>

            <div className="space-y-3">
              {/* Reconnect Tunnel Button */}
              <button
                type="button"
                onClick={() =>
                  handleAction(
                    'reconnect_tunnel',
                    reconnectTunnel,
                    'Reverse WebSocket tunnel reconnect sequence initiated'
                  )
                }
                disabled={isExecuting}
                data-testid="btn-reconnect-tunnel"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-lg bg-indigo-600/20 hover:bg-indigo-600/30 disabled:bg-slate-800/50 disabled:text-slate-600 text-indigo-300 text-xs font-medium transition border border-indigo-500/30"
              >
                <span className="flex items-center space-x-2">
                  {loadingAction === 'reconnect_tunnel' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-indigo-400" />
                  ) : (
                    <RefreshCw className="w-4 h-4" />
                  )}
                  <span>Reconnect Tunnel</span>
                </span>
                <span className="font-mono text-[10px] text-indigo-400">WS RESET</span>
              </button>
            </div>
          </div>

          <div className="mt-5 pt-3 border-t border-slate-800 flex justify-between items-center text-[11px] text-slate-500">
            <span>Tunnel: <strong className="text-slate-300">{state.tunnelState}</strong></span>
            <span>Shizuku: <strong className="text-slate-300">{state.shizukuStatus}</strong></span>
          </div>
        </div>
      </div>
    </div>
  );
};
