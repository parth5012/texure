import React, { useState, useEffect, useRef } from 'react';
import { useCompanion } from '../context/CompanionContext';
import {
  ShieldCheck,
  Radio,
  Wifi,
  WifiOff,
  Smartphone,
  Battery,
  BatteryCharging,
  BatteryWarning,
  Eye,
  EyeOff,
  Layers,
  RefreshCw,
  Activity,
  Server,
  Zap,
  Copy,
  Check,
  Cpu,
  Lock,
  ArrowRight,
} from 'lucide-react';
import { ShizukuStatus, TunnelState, AppStatus, NetworkType } from '../types';

const BATTERY_LOW_THRESHOLD = 20;
const BATTERY_MEDIUM_THRESHOLD = 50;
const BATTERY_HIGH_THRESHOLD = 90;

const sanitizeDisplayUrl = (url?: string): string => {
  if (!url) return 'wss://desktop.lan/tunnel';
  try {
    const parsed = new URL(url);
    if (parsed.protocol === 'ws:' || parsed.protocol === 'wss:') {
      parsed.searchParams.delete('token');
      return parsed.toString();
    }
  } catch {
    // ignore invalid URL
  }
  return 'wss://desktop.lan/tunnel';
};

const sanitizeDisplayHost = (host?: string): string => {
  if (!host) return 'desktop.lan';
  return host.replace(/[^\w.-]/g, '');
};

const ShizukuBadge: React.FC<{ status: ShizukuStatus }> = ({ status }) => {
  switch (status) {
    case 'AUTHORIZED':
      return (
        <span
          data-testid="shizuku-badge-authorized"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border border-emerald-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0" />
          <span>AUTHORIZED</span>
        </span>
      );
    case 'UNAUTHORIZED':
      return (
        <span
          data-testid="shizuku-badge-unauthorized"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-amber-500 shrink-0" />
          <span>UNAUTHORIZED</span>
        </span>
      );
    case 'SERVICE_DEAD':
    default:
      return (
        <span
          data-testid="shizuku-badge-dead"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-rose-500/10 text-rose-700 dark:text-rose-400 border border-rose-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-rose-500 shrink-0" />
          <span>SERVICE DEAD</span>
        </span>
      );
  }
};

const TunnelBadge: React.FC<{ tunnelState: TunnelState }> = ({ tunnelState }) => {
  switch (tunnelState) {
    case 'CONNECTED':
      return (
        <span
          data-testid="tunnel-badge-connected"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border border-emerald-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0" />
          <span>CONNECTED</span>
        </span>
      );
    case 'CONNECTING':
      return (
        <span
          data-testid="tunnel-badge-connecting"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-spin shrink-0" />
          <span>CONNECTING</span>
        </span>
      );
    case 'ERROR':
      return (
        <span
          data-testid="tunnel-badge-error"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-rose-500/10 text-rose-700 dark:text-rose-400 border border-rose-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-rose-500 shrink-0" />
          <span>TUNNEL ERROR</span>
        </span>
      );
    case 'DISCONNECTED':
    default:
      return (
        <span
          data-testid="tunnel-badge-disconnected"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-slate-500/10 text-slate-700 dark:text-zinc-400 border border-slate-300 dark:border-zinc-700 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-slate-400 shrink-0" />
          <span>DISCONNECTED</span>
        </span>
      );
  }
};

const AppStatusBadge: React.FC<{ appStatus: AppStatus }> = ({ appStatus }) => {
  switch (appStatus) {
    case 'FOREGROUND':
      return (
        <span
          data-testid="app-badge-foreground"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border border-emerald-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0" />
          <span>FOREGROUND</span>
        </span>
      );
    case 'BACKGROUND':
      return (
        <span
          data-testid="app-badge-background"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-500/25 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-amber-500 shrink-0" />
          <span>BACKGROUND</span>
        </span>
      );
    case 'STOPPED':
    default:
      return (
        <span
          data-testid="app-badge-stopped"
          className="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide bg-slate-500/10 text-slate-700 dark:text-zinc-400 border border-slate-300 dark:border-zinc-700 transition-all duration-200"
        >
          <span className="w-1.5 h-1.5 rounded-full bg-slate-400 shrink-0" />
          <span>STOPPED</span>
        </span>
      );
  }
};

const BatteryIcon: React.FC<{ percent: number }> = ({ percent }) => {
  if (percent <= BATTERY_LOW_THRESHOLD) return <BatteryWarning className="w-4 h-4 text-rose-500 dark:text-rose-400" />;
  if (percent >= BATTERY_HIGH_THRESHOLD) return <BatteryCharging className="w-4 h-4 text-emerald-500 dark:text-emerald-400" />;
  return <Battery className="w-4 h-4 text-indigo-500 dark:text-indigo-400" />;
};

const NetworkIcon: React.FC<{ type: NetworkType }> = ({ type }) => {
  if (type === 'WIFI') return <Wifi className="w-4 h-4 text-indigo-500 dark:text-indigo-400" />;
  if (type === 'CELLULAR') return <Zap className="w-4 h-4 text-amber-500 dark:text-amber-400" />;
  return <WifiOff className="w-4 h-4 text-slate-400 dark:text-zinc-500" />;
};

export const StatusDashboard: React.FC = () => {
  const { state, refreshState, reconnectTunnel } = useCompanion();
  const [refreshing, setRefreshing] = useState(false);
  const [copiedUrl, setCopiedUrl] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
      if (copyTimeoutRef.current) clearTimeout(copyTimeoutRef.current);
    };
  }, []);

  const handleCopyEndpoint = async (url: string) => {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(url);
        setCopiedUrl(true);
        if (copyTimeoutRef.current) clearTimeout(copyTimeoutRef.current);
        copyTimeoutRef.current = setTimeout(() => setCopiedUrl(false), 2000);
      }
    } catch {
      // ignore
    }
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await refreshState();
    } catch (err) {
      console.error('Failed to refresh device state:', err);
    } finally {
      timeoutRef.current = setTimeout(() => {
        setRefreshing(false);
      }, 350);
    }
  };

  const vitals = state.vitals ?? {
    batteryPercent: 100,
    screenOn: true,
    networkType: 'WIFI',
    latencyMs: 0,
  };
  const batteryPercent = vitals.batteryPercent ?? 0;
  const screenOn = vitals.screenOn ?? false;
  const networkType = vitals.networkType ?? 'NONE';
  const latencyMs = vitals.latencyMs ?? 0;

  const displayUrl = sanitizeDisplayUrl(state.pairing?.url);
  const displayHost = sanitizeDisplayHost(state.pairing?.host);

  return (
    <div className="space-y-6">
      {/* Title & Refresh Control */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900 dark:text-zinc-100 flex items-center space-x-2">
            <Activity className="w-5 h-5 text-indigo-500 dark:text-indigo-400" />
            <span>Companion Status Dashboard</span>
          </h2>
          <p className="text-xs text-slate-600 dark:text-zinc-400 mt-1">
            Real-time telemetry for Shizuku lifecycle gateway and reverse WebSocket tunnel.
          </p>
        </div>

        <button
          onClick={handleRefresh}
          disabled={refreshing}
          data-testid="dashboard-refresh-btn"
          className="inline-flex items-center space-x-2 px-3.5 py-1.5 rounded-xl bg-white dark:bg-zinc-900 hover:bg-slate-50 dark:hover:bg-zinc-800 border border-slate-300 dark:border-zinc-700 text-slate-800 dark:text-zinc-200 text-xs font-medium transition shadow-xs self-start sm:self-auto active:scale-[0.98]"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin text-indigo-500' : 'text-slate-400'}`} />
          <span>Refresh</span>
        </button>
      </div>

      {/* Grid of Status Cards with perfectly aligned headers and domain color encoding */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {/* Card 1: Shizuku Service Gateway (Domain: Indigo) */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div>
            <div className="flex items-center justify-between mb-2.5">
              <div className="p-1.5 rounded-lg bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/40 shrink-0">
                <ShieldCheck className="w-4 h-4" />
              </div>
              <ShizukuBadge status={state.shizukuStatus} />
            </div>
            <h3 className="text-sm font-semibold text-slate-900 dark:text-zinc-100 tracking-tight whitespace-nowrap mb-3.5">
              Shizuku Gateway
            </h3>

            <div className="space-y-2.5 text-[13px] text-slate-600 dark:text-zinc-300">
              <div className="flex justify-between py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Service Version</span>
                <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold">
                  {state.shizukuStatus === 'AUTHORIZED' ? 'IPC v13' : 'Offline'}
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Shell UID</span>
                <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold">
                  {state.shizukuStatus === 'AUTHORIZED' ? 'UID 2000 (Shell)' : 'Unprivileged'}
                </span>
              </div>
              <div className="flex justify-between py-1">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Permission Hook</span>
                <span className={`font-mono font-semibold flex items-center space-x-1.5 ${
                  state.shizukuStatus === 'AUTHORIZED'
                    ? 'text-emerald-700 dark:text-emerald-400'
                    : state.shizukuStatus === 'UNAUTHORIZED'
                    ? 'text-amber-700 dark:text-amber-400'
                    : 'text-rose-700 dark:text-rose-400'
                }`}>
                  <span className={`w-1.5 h-1.5 rounded-full ${
                    state.shizukuStatus === 'AUTHORIZED'
                      ? 'bg-emerald-500'
                      : state.shizukuStatus === 'UNAUTHORIZED'
                      ? 'bg-amber-500'
                      : 'bg-rose-500'
                  }`} />
                  <span>
                    {state.shizukuStatus === 'AUTHORIZED' ? 'Active' : state.shizukuStatus === 'UNAUTHORIZED' ? 'Declined' : 'Service Dead'}
                  </span>
                </span>
              </div>
            </div>
          </div>
          <div className="mt-4 pt-2.5 border-t border-slate-100 dark:border-zinc-800/60 text-[11px] font-medium text-slate-500 dark:text-zinc-500 flex items-center space-x-1.5">
            <Lock className="w-3.5 h-3.5 opacity-60" />
            <span>Lifecycle &amp; Keyguard</span>
          </div>
        </div>

        {/* Card 2: Reverse WebSocket Tunnel (Domain: Sky) */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div>
            <div className="flex items-center justify-between mb-2.5">
              <div className="p-1.5 rounded-lg bg-sky-50 dark:bg-sky-950/60 text-sky-600 dark:text-sky-400 border border-sky-200/60 dark:border-sky-800/40 shrink-0">
                <Radio className="w-4 h-4" />
              </div>
              <TunnelBadge tunnelState={state.tunnelState} />
            </div>
            <h3 className="text-sm font-semibold text-slate-900 dark:text-zinc-100 tracking-tight whitespace-nowrap mb-3.5">
              Reverse WS Tunnel
            </h3>

            <div className="space-y-2.5 text-[13px] text-slate-600 dark:text-zinc-300">
              <div className="flex justify-between items-center py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Endpoint</span>
                <div className="flex items-center space-x-1.5">
                  <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold truncate max-w-[120px]" title={displayUrl}>
                    {displayHost}
                  </span>
                  <button
                    onClick={() => handleCopyEndpoint(displayUrl)}
                    title="Copy Full Endpoint URL"
                    data-testid="btn-copy-endpoint"
                    className="p-1 rounded-md hover:bg-slate-100 dark:hover:bg-zinc-800 text-slate-400 hover:text-slate-700 dark:hover:text-zinc-200 transition"
                  >
                    {copiedUrl ? (
                      <Check className="w-3.5 h-3.5 text-emerald-500" />
                    ) : (
                      <Copy className="w-3.5 h-3.5" />
                    )}
                  </button>
                </div>
              </div>

              <div className="flex justify-between py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Round-trip Ping</span>
                <span className="font-mono text-emerald-700 dark:text-emerald-400 font-semibold">
                  {state.tunnelState === 'CONNECTED' ? `${latencyMs} ms` : '—'}
                </span>
              </div>

              <div className="flex justify-between py-1">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Auth Token</span>
                <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold">
                  {state.pairing?.token ? 'Bearer Verified' : 'Unpaired'}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-2.5 border-t border-slate-100 dark:border-zinc-800/60 flex items-center justify-between text-xs">
            <span className="text-slate-500 dark:text-zinc-400">Supervisor: <strong className="font-mono font-medium text-slate-800 dark:text-zinc-200">{state.tunnelState === 'CONNECTED' ? 'Active' : state.tunnelState}</strong></span>
            <button
              onClick={() => reconnectTunnel()}
              className="px-2.5 py-1 rounded-lg bg-indigo-50 hover:bg-indigo-100 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-300 font-semibold border border-indigo-200/80 dark:border-indigo-800/60 transition shadow-xs text-xs"
            >
              Reconnect
            </button>
          </div>
        </div>

        {/* Card 3: Target Application (Domain: Emerald) */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div>
            <div className="flex items-center justify-between mb-2.5">
              <div className="p-1.5 rounded-lg bg-emerald-50 dark:bg-emerald-950/60 text-emerald-600 dark:text-emerald-400 border border-emerald-200/60 dark:border-emerald-800/40 shrink-0">
                <Layers className="w-4 h-4" />
              </div>
              <AppStatusBadge appStatus={state.appStatus} />
            </div>
            <h3 className="text-sm font-semibold text-slate-900 dark:text-zinc-100 tracking-tight whitespace-nowrap mb-3.5">
              Target Application
            </h3>

            <div className="space-y-2.5 text-[13px] text-slate-600 dark:text-zinc-300">
              <div className="flex justify-between py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Package</span>
                <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold" title="com.example.target">
                  com.example.target
                </span>
              </div>

              <div className="flex justify-between py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Activity</span>
                <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold">.MainActivity</span>
              </div>

              <div className="flex justify-between py-1">
                <span className="text-slate-600 dark:text-zinc-400 font-medium">Bridge Link</span>
                <span className={`font-mono font-semibold ${
                  state.appStatus === 'FOREGROUND'
                    ? 'text-emerald-700 dark:text-emerald-400'
                    : state.appStatus === 'BACKGROUND'
                    ? 'text-amber-700 dark:text-amber-400'
                    : 'text-slate-500 dark:text-zinc-500'
                }`}>
                  {state.appStatus === 'FOREGROUND' ? '127.0.0.1:8765' : (state.appStatus === 'BACKGROUND' ? 'Paused' : 'Offline')}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-2.5 border-t border-slate-100 dark:border-zinc-800/60 text-[11px] font-medium text-slate-500 dark:text-zinc-500 flex items-center space-x-1.5">
            <Cpu className="w-3.5 h-3.5 opacity-60" />
            <span>Process State &amp; Lifecycle</span>
          </div>
        </div>

        {/* Card 4: Device Vitals & Telemetry (Domain: Violet) */}
        <div className="glass-panel rounded-2xl p-5 flex flex-col justify-between transition-all duration-200">
          <div>
            <div className="flex items-center justify-between mb-2.5">
              <div className="p-1.5 rounded-lg bg-violet-50 dark:bg-violet-950/60 text-violet-600 dark:text-violet-400 border border-violet-200/60 dark:border-violet-800/40 shrink-0">
                <Smartphone className="w-4 h-4" />
              </div>
              <span className={`inline-flex items-center space-x-1.5 px-2 py-0.5 rounded-full text-[11px] font-semibold tracking-wide border ${
                batteryPercent < 15
                  ? 'bg-rose-500/10 text-rose-700 dark:text-rose-400 border-rose-500/25'
                  : batteryPercent < 30
                  ? 'bg-amber-500/10 text-amber-700 dark:text-amber-400 border-amber-500/25'
                  : 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-400 border-emerald-500/25'
              }`}>
                <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                  batteryPercent < 15 ? 'bg-rose-500' : batteryPercent < 30 ? 'bg-amber-500' : 'bg-emerald-500'
                }`} />
                <span>{networkType} • {batteryPercent}%</span>
              </span>
            </div>
            <h3 className="text-sm font-semibold text-slate-900 dark:text-zinc-100 tracking-tight whitespace-nowrap mb-3.5">
              Device Vitals
            </h3>

            <div className="space-y-2.5 text-[13px] text-slate-600 dark:text-zinc-300">
              <div className="flex justify-between items-center py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium flex items-center space-x-1.5">
                  <BatteryIcon percent={batteryPercent} />
                  <span>Battery</span>
                </span>
                <div className="flex items-center space-x-2">
                  <div className="w-14 h-2 bg-slate-200 dark:bg-zinc-800 rounded-full overflow-hidden p-0.5 ring-1 ring-slate-300 dark:ring-zinc-700">
                    <div
                      className={`h-full rounded-full transition-all duration-500 ${
                        batteryPercent <= BATTERY_LOW_THRESHOLD
                          ? 'bg-rose-500'
                          : batteryPercent <= BATTERY_MEDIUM_THRESHOLD
                          ? 'bg-amber-500'
                          : 'bg-emerald-500'
                      }`}
                      style={{ width: `${Math.max(4, batteryPercent)}%` }}
                    />
                  </div>
                  <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold text-xs">
                    {batteryPercent}%
                  </span>
                </div>
              </div>

              <div className="flex justify-between items-center py-1 border-b border-slate-100 dark:border-zinc-800/60">
                <span className="text-slate-600 dark:text-zinc-400 font-medium flex items-center space-x-1.5">
                  {screenOn ? (
                    <Eye className="w-4 h-4 text-emerald-500 dark:text-emerald-400" />
                  ) : (
                    <EyeOff className="w-4 h-4 text-rose-500 dark:text-rose-400" />
                  )}
                  <span>Screen State</span>
                </span>
                <span
                  data-testid="screen-state-indicator"
                  className={`font-semibold font-mono text-xs px-2 py-0.5 rounded ${
                    screenOn
                      ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-400'
                      : 'bg-rose-500/10 text-rose-700 dark:text-rose-400'
                  }`}
                >
                  {screenOn ? 'ON' : 'OFF'}
                </span>
              </div>

              <div className="flex justify-between items-center py-1">
                <span className="text-slate-600 dark:text-zinc-400 font-medium flex items-center space-x-1.5">
                  <NetworkIcon type={networkType} />
                  <span>Transport</span>
                </span>
                <span className="font-mono text-slate-900 dark:text-zinc-100 font-semibold">{networkType}</span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-2.5 border-t border-slate-100 dark:border-zinc-800/60 text-[11px] font-medium text-slate-500 dark:text-zinc-500 flex items-center space-x-1.5">
            <Activity className="w-3.5 h-3.5 opacity-60" />
            <span>Hardware Telemetry Stream</span>
          </div>
        </div>
      </div>

      {/* Expanded Infrastructure Summary - High-End Observatory Network Topology */}
      <div className="glass-panel rounded-2xl p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100 flex items-center space-x-2">
            <Server className="w-4 h-4 text-indigo-500 dark:text-indigo-400" />
            <span>Active Gateway Topology</span>
          </h3>
          <span className="text-xs font-mono text-emerald-700 dark:text-emerald-400 font-semibold px-2.5 py-0.5 rounded-full bg-emerald-500/10 border border-emerald-500/20">
            TLS ENCRYPTED
          </span>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 items-center gap-4 pt-2">
          {/* Node 1: Android Host Device */}
          <div className="bg-slate-50/90 dark:bg-zinc-950/80 p-4 rounded-xl border border-slate-200/80 dark:border-zinc-800/80 flex items-center space-x-3.5 shadow-xs">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-indigo-500 to-indigo-600 flex items-center justify-center text-white shrink-0 shadow-sm shadow-indigo-500/20">
              <Smartphone className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="font-semibold text-slate-900 dark:text-zinc-100 text-xs">Android Host Device</div>
              <div className="text-xs text-slate-600 dark:text-zinc-400 font-mono mt-0.5">
                {state.pairing ? 'Paired Direct Link' : 'Local Interface'}
              </div>
              <div className="text-xs text-emerald-700 dark:text-emerald-400 font-medium mt-1">
                {state.shizukuStatus === 'AUTHORIZED' ? 'Shizuku Service Hooked' : 'Awaiting Shizuku'}
              </div>
            </div>
          </div>

          {/* Central Conduit / Stream Flow */}
          <div className="flex flex-col items-center justify-center px-4 py-2">
            <div className="flex items-center space-x-2 text-xs font-mono font-semibold text-indigo-600 dark:text-indigo-400 mb-1.5">
              <span>wss://</span>
              <span className="text-slate-400 dark:text-zinc-600">•</span>
              <span className="text-emerald-700 dark:text-emerald-400">{latencyMs} ms RTT</span>
            </div>

            <div className="w-full relative h-2 bg-slate-200/80 dark:bg-zinc-800 rounded-full overflow-hidden">
              {/* Persistent base gradient so conduit is never an empty progress bar */}
              <div className="absolute inset-0 bg-gradient-to-r from-indigo-500/40 via-violet-500/40 to-emerald-400/40" />
              {/* Active data packet sweep */}
              <div className="absolute inset-0 bg-gradient-to-r from-transparent via-indigo-400/90 to-emerald-300/90 animate-stream" />
            </div>

            <div className="flex items-center space-x-1.5 text-xs text-slate-500 dark:text-zinc-400 mt-1.5 font-mono">
              <span>REVERSE GATEWAY</span>
              <ArrowRight className="w-3.5 h-3.5 text-indigo-500" />
            </div>
          </div>

          {/* Node 2: Desktop Bridge */}
          <div className="bg-slate-50/90 dark:bg-zinc-950/80 p-4 rounded-xl border border-slate-200/80 dark:border-zinc-800/80 flex items-center space-x-3.5 shadow-xs">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-emerald-500 to-teal-600 flex items-center justify-center text-white shrink-0 shadow-sm shadow-emerald-500/20">
              <Radio className="w-5 h-5" />
            </div>
            <div className="min-w-0">
              <div className="font-semibold text-slate-900 dark:text-zinc-100 text-xs">Desktop MCP Server</div>
              <div className="text-xs text-slate-600 dark:text-zinc-400 font-mono mt-0.5">
                {displayHost !== '—' ? `${displayHost} (Gateway)` : 'Gateway Standby'}
              </div>
              <div className="text-xs text-emerald-700 dark:text-emerald-400 font-medium mt-1">
                {state.tunnelState === 'CONNECTED' ? 'MCP Protocol Active' : 'Tunnel Inactive'}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
