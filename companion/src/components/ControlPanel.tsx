import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { createPortal } from 'react-dom';
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

const DEFAULT_PACKAGE_NAME = (import.meta as { env?: Record<string, string> }).env?.VITE_DEFAULT_TARGET_PACKAGE || 'com.example.target';
const TOAST_DURATION_MS = 4000;

const sanitizeToastText = (raw: string): string => {
  return raw.replace(/<[^>]*>?/gm, '').slice(0, 300);
};

const getToastStyles = (type: 'success' | 'error' | 'info'): string => {
  switch (type) {
    case 'success':
      return 'bg-white/95 dark:bg-zinc-900/95 border-emerald-500/40 text-emerald-700 dark:text-emerald-300 shadow-emerald-500/10';
    case 'error':
      return 'bg-white/95 dark:bg-zinc-900/95 border-rose-500/40 text-rose-700 dark:text-rose-300 shadow-rose-500/10';
    case 'info':
    default:
      return 'bg-white/95 dark:bg-zinc-900/95 border-indigo-500/40 text-indigo-700 dark:text-indigo-300 shadow-indigo-500/10';
  }
};

export const ANDROID_PACKAGE_REGEX = /^[a-zA-Z][a-zA-Z0-9_]*(\.[a-zA-Z][a-zA-Z0-9_]*)+$/;

export const ControlPanel: React.FC = () => {
  const {
    state,
    wakeScreen,
    dismissKeyguard,
    coldStartApp,
    forceStopApp,
    reconnectTunnel,
    refreshState,
  } = useCompanion();

  const [packageName, setPackageName] = useState<string>(DEFAULT_PACKAGE_NAME);
  const [loadingAction, setLoadingAction] = useState<string | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastTimeoutsRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const dispatchingRef = useRef(false);

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
    if (dispatchingRef.current || loadingAction) return;
    dispatchingRef.current = true;
    setLoadingAction(actionId);

    try {
      await fn();
      addToast('success', 'Action Executed', successMsg);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      addToast('error', 'Action Failed', errorMsg);
    } finally {
      dispatchingRef.current = false;
      setLoadingAction(null);
    }
  };

  const isExecuting = loadingAction !== null;
  const effectivePackage = packageName.trim() || DEFAULT_PACKAGE_NAME;
  const isPackageValid = useMemo(() => {
    return ANDROID_PACKAGE_REGEX.test(effectivePackage) && effectivePackage.length <= 255;
  }, [effectivePackage]);

  return (
    <div data-testid="control-panel" className="space-y-6">
      {/* Toast Notification Deck - Portaled to document.body to escape transform containing blocks */}
      {typeof document !== 'undefined' &&
        createPortal(
          <aside aria-label="Notifications" className="fixed top-[9.5rem] right-6 z-50 flex flex-col space-y-2.5 max-w-sm w-full pointer-events-none">
            {toasts.map((toast) => (
              <div
                key={toast.id}
                data-testid="toast-notification"
                className={`pointer-events-auto p-4 rounded-2xl border shadow-xl backdrop-blur-xl flex items-start space-x-3 transition-all transform animate-toast-slide ${getToastStyles(
                  toast.type
                )}`}
              >
                {toast.type === 'success' && <CheckCircle2 className="w-5 h-5 text-emerald-500 shrink-0 mt-0.5" />}
                {toast.type === 'error' && <AlertCircle className="w-5 h-5 text-rose-500 shrink-0 mt-0.5" />}
                {toast.type === 'info' && <Info className="w-5 h-5 text-indigo-500 shrink-0 mt-0.5" />}
                <div className="flex-1 min-w-0">
                  <h4 className="text-xs font-semibold text-slate-900 dark:text-zinc-100">{toast.title}</h4>
                  <p data-testid="toast-message" className="text-xs text-slate-600 dark:text-zinc-300 mt-0.5 break-words">
                    {toast.message}
                  </p>
                </div>
                <button
                  onClick={() => dismissToast(toast.id)}
                  data-testid="toast-dismiss"
                  className="text-slate-400 hover:text-slate-700 dark:hover:text-zinc-200 transition shrink-0 p-1 rounded-md"
                  aria-label="Dismiss notification"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
            ))}
          </aside>,
          document.body
        )}

      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900 dark:text-zinc-100 flex items-center space-x-2">
            <Sliders className="w-5 h-5 text-indigo-500 dark:text-indigo-400" />
            <span>Device Manual Control Panel</span>
          </h2>
          <p className="text-xs text-slate-600 dark:text-zinc-400 mt-1">
            Direct hardware &amp; lifecycle override commands executed via Shizuku shell IPC and WebSocket bridge.
          </p>
        </div>

        {/* Global Dispatch Status Indicator */}
        <div className="flex items-center space-x-2 px-3 py-1.5 rounded-full bg-white dark:bg-zinc-900 border border-slate-200/90 dark:border-zinc-800 shadow-xs self-start sm:self-auto">
          <span
            className={`w-2 h-2 rounded-full ${
              isExecuting ? 'bg-amber-400 animate-ping' : 'bg-emerald-500'
            }`}
          />
          <span className="font-mono text-slate-700 dark:text-zinc-300 font-semibold tracking-wider text-[11px]">
            {isExecuting ? 'COMMAND RUNNING' : 'IDLE / READY'}
          </span>
        </div>
      </div>

      {/* Action Cards Grid - Equalized Height & Information Density */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Section 1: Screen & Display Controls */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div className="flex-1 flex flex-col">
            <div className="flex items-center space-x-2.5 mb-2">
              <div className="p-1.5 rounded-lg bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/40">
                <Smartphone className="w-4 h-4" />
              </div>
              <h3 className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100">Screen &amp; Keyguard</h3>
            </div>
            <p className="text-xs text-slate-600 dark:text-zinc-400 mb-4">
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
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bg-gradient-to-r from-indigo-500 to-indigo-600 hover:from-indigo-400 hover:to-indigo-500 disabled:opacity-50 text-white text-xs font-medium transition-all shadow-sm shadow-indigo-500/20 active:scale-[0.99]"
              >
                <span className="flex items-center space-x-2.5">
                  {loadingAction === 'wake_screen' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-white" />
                  ) : (
                    <Sun className="w-4 h-4" />
                  )}
                  <span>Wake Screen</span>
                </span>
                <span className="font-mono text-xs uppercase tracking-wider px-2 py-0.5 rounded bg-black/20 text-white/95 font-semibold">
                  KEYEVENT 26
                </span>
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
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bg-slate-100 hover:bg-slate-200/90 dark:bg-zinc-800 dark:hover:bg-zinc-700/80 disabled:opacity-50 text-slate-800 dark:text-zinc-100 text-xs font-medium transition-all border border-slate-300 dark:border-zinc-600 shadow-xs active:scale-[0.99]"
              >
                <span className="flex items-center space-x-2.5">
                  {loadingAction === 'dismiss_keyguard' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-indigo-500" />
                  ) : (
                    <Unlock className="w-4 h-4 text-indigo-600 dark:text-indigo-400" />
                  )}
                  <span>Dismiss Keyguard</span>
                </span>
                <span className="font-mono text-xs text-slate-600 dark:text-zinc-400 font-medium">wm unlock</span>
              </button>
            </div>

            {/* Display & Power Telemetry - Dynamic Diagnostics */}
            <div className="mt-4 flex-1 flex flex-col justify-center p-3 rounded-xl bg-slate-50/70 dark:bg-zinc-950/40 border border-slate-200/60 dark:border-zinc-800/60 space-y-2 text-xs">
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Display State</span>
                <span className="font-mono text-slate-800 dark:text-zinc-200 font-medium flex items-center space-x-1.5">
                  <span className={`w-1.5 h-1.5 rounded-full ${state.vitals.screenOn ? 'bg-emerald-500' : 'bg-slate-400'}`} />
                  <span>{state.vitals.screenOn ? 'AWAKE / INTERACTIVE' : 'ASLEEP'}</span>
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Keyguard Lock</span>
                <span className={`font-mono font-medium ${state.vitals.screenOn ? 'text-emerald-700 dark:text-emerald-400' : 'text-amber-700 dark:text-amber-400'}`}>
                  {state.vitals.screenOn ? 'DISMISSED / UNLOCKED' : 'ENGAGED / LOCKED'}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Input Dispatcher</span>
                <span className="font-mono text-slate-800 dark:text-zinc-200 font-medium">
                  {state.shizukuStatus === 'AUTHORIZED' ? 'ADB_SHELL (UID 2000)' : 'UNPRIVILEGED'}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-3 border-t border-slate-100 dark:border-zinc-800/60 text-xs text-slate-500 dark:text-zinc-400 flex justify-between items-center">
            <span>Shell Protocol</span>
            <strong className="font-mono text-xs text-emerald-700 dark:text-emerald-400 font-semibold">
              Shizuku Direct IPC
            </strong>
          </div>
        </div>

        {/* Section 2: Application Lifecycle Controls */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div className="flex-1 flex flex-col">
            <div className="flex items-center space-x-2.5 mb-2">
              <div className="p-1.5 rounded-lg bg-emerald-50 dark:bg-emerald-950/60 text-emerald-600 dark:text-emerald-400 border border-emerald-200/60 dark:border-emerald-800/40">
                <Layers className="w-4 h-4" />
              </div>
              <h3 className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100">Application Lifecycle</h3>
            </div>
            <p className="text-xs text-slate-600 dark:text-zinc-400 mb-3">
              Trigger cold launch or force-stop for the target application package.
            </p>

            {/* Target Package Input */}
            <div className="mb-3">
              <label htmlFor="target-package-input" className="block text-xs font-semibold text-slate-700 dark:text-zinc-300 mb-1.5 uppercase tracking-wider">
                Target Package
              </label>
              <div className="relative">
                <input
                  id="target-package-input"
                  type="text"
                  value={packageName}
                  onChange={(e) => setPackageName(e.target.value.trim())}
                  data-testid="input-package-name"
                  className={`w-full px-3.5 py-2 bg-slate-50 dark:bg-zinc-950 border rounded-xl text-xs font-mono text-slate-900 dark:text-zinc-100 placeholder-slate-500 dark:placeholder-zinc-500 focus:outline-none transition-all ${
                    isPackageValid
                      ? 'border-slate-300 dark:border-zinc-700 focus:border-indigo-500 dark:focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500'
                      : 'border-rose-400 focus:border-rose-500 focus:ring-1 focus:ring-rose-500'
                  }`}
                  placeholder="com.example.target"
                />
              </div>
              {!isPackageValid && (
                <p data-testid="package-validation-error" className="text-xs text-rose-700 dark:text-rose-400 font-medium mt-1">
                  Invalid package name. Must be a valid Java/Android package (e.g. com.example.app).
                </p>
              )}
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
                disabled={isExecuting || !isPackageValid}
                data-testid="btn-cold-start"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bg-gradient-to-r from-emerald-500 to-emerald-600 hover:from-emerald-400 hover:to-emerald-500 disabled:opacity-50 text-white text-xs font-medium transition-all shadow-sm shadow-emerald-500/20 active:scale-[0.99]"
              >
                <span className="flex items-center space-x-2.5">
                  {loadingAction === 'cold_start' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-white" />
                  ) : (
                    <Play className="w-4 h-4 fill-white" />
                  )}
                  <span>Cold Start App</span>
                </span>
                <span className="font-mono text-xs uppercase tracking-wider px-2 py-0.5 rounded bg-black/20 text-white/95 font-semibold">
                  am start
                </span>
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
                disabled={isExecuting || !isPackageValid}
                data-testid="btn-force-stop"
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bg-rose-50 hover:bg-rose-100 dark:bg-rose-500/10 dark:hover:bg-rose-500/20 disabled:opacity-50 text-rose-700 dark:text-rose-400 text-xs font-medium transition-all border border-rose-500 hover:border-rose-600 dark:border-rose-500/40 active:scale-[0.99]"
              >
                <span className="flex items-center space-x-2.5">
                  {loadingAction === 'force_stop' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-rose-500" />
                  ) : (
                    <Square className="w-4 h-4 fill-rose-500" />
                  )}
                  <span>Force Stop App</span>
                </span>
                <span className="font-mono text-xs text-rose-700 dark:text-rose-400 font-medium">am force-stop</span>
              </button>
            </div>

            {/* Application Process & Lifecycle Telemetry */}
            <div className="mt-4 flex-1 flex flex-col justify-center p-3 rounded-xl bg-slate-50/70 dark:bg-zinc-950/40 border border-slate-200/60 dark:border-zinc-800/60 space-y-2 text-xs">
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Process State</span>
                <span className="font-mono text-slate-800 dark:text-zinc-200 font-medium flex items-center space-x-1.5">
                  <span className={`w-1.5 h-1.5 rounded-full ${state.appStatus === 'FOREGROUND' ? 'bg-emerald-500' : state.appStatus === 'BACKGROUND' ? 'bg-amber-500' : 'bg-slate-400'}`} />
                  <span>{state.appStatus}</span>
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Process Binding</span>
                <span className={`font-mono font-medium ${state.appStatus === 'FOREGROUND' ? 'text-emerald-700 dark:text-emerald-400' : 'text-slate-500 dark:text-zinc-500'}`}>
                  {state.appStatus === 'FOREGROUND' ? 'ATTACHED (ACTIVE)' : 'DETACHED'}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">IPC Transport</span>
                <span className="font-mono text-slate-800 dark:text-zinc-200 font-medium">UNIX Socket</span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-3 border-t border-slate-100 dark:border-zinc-800/60 text-xs text-slate-500 dark:text-zinc-400 flex justify-between items-center">
            <span>Process Status</span>
            <strong className="font-mono text-slate-800 dark:text-zinc-200 text-xs">{state.appStatus}</strong>
          </div>
        </div>

        {/* Section 3: Connection & WebSocket Tunnel */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div className="flex-1 flex flex-col">
            <div className="flex items-center space-x-2.5 mb-2">
              <div className="p-1.5 rounded-lg bg-sky-50 dark:bg-sky-950/60 text-sky-600 dark:text-sky-400 border border-sky-200/60 dark:border-sky-800/40">
                <Radio className="w-4 h-4" />
              </div>
              <h3 className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100">Reverse WS Tunnel</h3>
            </div>
            <p className="text-xs text-slate-600 dark:text-zinc-400 mb-4">
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
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bg-indigo-50 hover:bg-indigo-100 dark:bg-indigo-950/60 dark:hover:bg-indigo-900/60 disabled:opacity-50 text-indigo-700 dark:text-indigo-300 text-xs font-semibold transition-all border border-indigo-500 hover:border-indigo-600 dark:border-indigo-500 shadow-xs active:scale-[0.99]"
              >
                <span className="flex items-center space-x-2.5">
                  {loadingAction === 'reconnect_tunnel' ? (
                    <RefreshCw className="w-4 h-4 animate-spin text-indigo-500" />
                  ) : (
                    <RefreshCw className="w-4 h-4" />
                  )}
                  <span>Reconnect Tunnel</span>
                </span>
                <span className="font-mono text-xs text-indigo-700 dark:text-indigo-400 font-semibold">WS RESET</span>
              </button>

              {/* Poll Tunnel Heartbeat Button */}
              <button
                type="button"
                onClick={() =>
                  handleAction(
                    'poll_heartbeat',
                    refreshState,
                    'WebSocket keep-alive ping dispatched'
                  )
                }
                disabled={isExecuting}
                className="w-full flex items-center justify-between px-4 py-2.5 rounded-xl bg-slate-100 hover:bg-slate-200/90 dark:bg-zinc-800 dark:hover:bg-zinc-700/80 disabled:opacity-50 text-slate-800 dark:text-zinc-100 text-xs font-medium transition-all border border-slate-300 dark:border-zinc-600 shadow-xs active:scale-[0.99]"
              >
                <span className="flex items-center space-x-2.5">
                  <Radio className="w-4 h-4 text-sky-500" />
                  <span>Poll Keep-Alive Ping</span>
                </span>
                <span className="font-mono text-xs text-slate-600 dark:text-zinc-400 font-medium">ping</span>
              </button>
            </div>

            {/* Real Tunnel State Telemetry */}
            <div className="mt-4 flex-1 flex flex-col justify-center p-3 rounded-xl bg-slate-50/70 dark:bg-zinc-950/40 border border-slate-200/60 dark:border-zinc-800/60 space-y-2 text-xs">
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Tunnel State</span>
                <span className={`font-mono font-medium flex items-center space-x-1.5 ${
                  state.tunnelState === 'CONNECTED' ? 'text-emerald-700 dark:text-emerald-400' : 'text-amber-700 dark:text-amber-400'
                }`}>
                  <span className={`w-1.5 h-1.5 rounded-full ${state.tunnelState === 'CONNECTED' ? 'bg-emerald-500' : 'bg-amber-500'}`} />
                  <span>{state.tunnelState === 'CONNECTED' ? 'ESTABLISHED (SECURE)' : state.tunnelState}</span>
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Keep-Alive RTT</span>
                <span className="font-mono font-medium text-emerald-700 dark:text-emerald-400">
                  {state.vitals.latencyMs ? `${state.vitals.latencyMs} ms` : '—'}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Channel Security</span>
                <span className="font-mono font-medium text-slate-800 dark:text-zinc-200">
                  {state.pairing?.token ? 'Bearer Encrypted' : 'Unpaired'}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-3 border-t border-slate-100 dark:border-zinc-800/60 flex justify-between items-center text-xs text-slate-500 dark:text-zinc-400">
            <span>Tunnel: <strong className="text-slate-800 dark:text-zinc-200 font-mono">{state.tunnelState}</strong></span>
            <span>Shizuku: <strong className="text-slate-800 dark:text-zinc-200 font-mono">{state.shizukuStatus}</strong></span>
          </div>
        </div>
      </div>
    </div>
  );
};
