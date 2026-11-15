import React, { useState } from 'react';
import { useCompanion } from '../context/CompanionContext';
import {
  ShieldCheck,
  ShieldAlert,
  ShieldX,
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
} from 'lucide-react';
import { ShizukuStatus, TunnelState, AppStatus, NetworkType } from '../types';

export const StatusDashboard: React.FC = () => {
  const { state, refreshState, reconnectTunnel } = useCompanion();
  const [refreshing, setRefreshing] = useState(false);

  const handleRefresh = async () => {
    setRefreshing(true);
    await refreshState();
    setTimeout(() => setRefreshing(false), 400);
  };

  // Badge renderers with semantic colors and icons
  const renderShizukuBadge = (status: ShizukuStatus) => {
    switch (status) {
      case 'AUTHORIZED':
        return (
          <span
            data-testid="shizuku-badge-authorized"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 transition-all duration-300"
          >
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>AUTHORIZED</span>
          </span>
        );
      case 'UNAUTHORIZED':
        return (
          <span
            data-testid="shizuku-badge-unauthorized"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-400 border border-amber-500/30 transition-all duration-300"
          >
            <ShieldAlert className="w-3.5 h-3.5" />
            <span>UNAUTHORIZED</span>
          </span>
        );
      case 'SERVICE_DEAD':
      default:
        return (
          <span
            data-testid="shizuku-badge-dead"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-rose-500/10 text-rose-400 border border-rose-500/30 transition-all duration-300"
          >
            <ShieldX className="w-3.5 h-3.5" />
            <span>SERVICE DEAD</span>
          </span>
        );
    }
  };

  const renderTunnelBadge = (tunnelState: TunnelState) => {
    switch (tunnelState) {
      case 'CONNECTED':
        return (
          <span
            data-testid="tunnel-badge-connected"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 transition-all duration-300"
          >
            <Radio className="w-3.5 h-3.5 animate-pulse text-emerald-400" />
            <span>CONNECTED</span>
          </span>
        );
      case 'CONNECTING':
        return (
          <span
            data-testid="tunnel-badge-connecting"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-400 border border-amber-500/30 transition-all duration-300"
          >
            <RefreshCw className="w-3.5 h-3.5 animate-spin text-amber-400" />
            <span>CONNECTING</span>
          </span>
        );
      case 'ERROR':
        return (
          <span
            data-testid="tunnel-badge-error"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-rose-500/10 text-rose-400 border border-rose-500/30 transition-all duration-300"
          >
            <WifiOff className="w-3.5 h-3.5" />
            <span>TUNNEL ERROR</span>
          </span>
        );
      case 'DISCONNECTED':
      default:
        return (
          <span
            data-testid="tunnel-badge-disconnected"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-slate-500/10 text-slate-400 border border-slate-700 transition-all duration-300"
          >
            <WifiOff className="w-3.5 h-3.5" />
            <span>DISCONNECTED</span>
          </span>
        );
    }
  };

  const renderAppStatusBadge = (appStatus: AppStatus) => {
    switch (appStatus) {
      case 'FOREGROUND':
        return (
          <span
            data-testid="app-badge-foreground"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 transition-all duration-300"
          >
            <Layers className="w-3.5 h-3.5" />
            <span>FOREGROUND</span>
          </span>
        );
      case 'BACKGROUND':
        return (
          <span
            data-testid="app-badge-background"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-amber-500/10 text-amber-400 border border-amber-500/30 transition-all duration-300"
          >
            <Layers className="w-3.5 h-3.5" />
            <span>BACKGROUND</span>
          </span>
        );
      case 'STOPPED':
      default:
        return (
          <span
            data-testid="app-badge-stopped"
            className="inline-flex items-center space-x-1 px-2.5 py-1 rounded-full text-xs font-semibold bg-slate-500/10 text-slate-400 border border-slate-700 transition-all duration-300"
          >
            <Layers className="w-3.5 h-3.5" />
            <span>STOPPED</span>
          </span>
        );
    }
  };

  const renderBatteryIcon = (percent: number) => {
    if (percent <= 20) return <BatteryWarning className="w-4 h-4 text-rose-400" />;
    if (percent >= 90) return <BatteryCharging className="w-4 h-4 text-emerald-400" />;
    return <Battery className="w-4 h-4 text-indigo-400" />;
  };

  const renderNetworkIcon = (type: NetworkType) => {
    if (type === 'WIFI') return <Wifi className="w-4 h-4 text-indigo-400" />;
    if (type === 'CELLULAR') return <Zap className="w-4 h-4 text-amber-400" />;
    return <WifiOff className="w-4 h-4 text-slate-500" />;
  };

  return (
    <div className="space-y-6">
      {/* Title & Refresh Control */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-bold tracking-tight text-slate-100 flex items-center space-x-2">
            <Activity className="w-5 h-5 text-indigo-400" />
            <span>Companion Status Dashboard</span>
          </h2>
          <p className="text-xs text-slate-400 mt-1">
            Real-time telemetry for Shizuku lifecycle gateway and reverse WebSocket tunnel.
          </p>
        </div>

        <button
          onClick={handleRefresh}
          disabled={refreshing}
          data-testid="dashboard-refresh-btn"
          className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 text-xs font-medium transition shadow-sm"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin text-indigo-400' : ''}`} />
          <span>Refresh</span>
        </button>
      </div>

      {/* Grid of Status Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Card 1: Shizuku Service Gateway */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 flex flex-col justify-between shadow-lg">
          <div>
            <div className="flex items-center justify-between mb-3">
              <span className="text-xs font-medium text-slate-400">Shizuku Gateway</span>
              {renderShizukuBadge(state.shizukuStatus)}
            </div>
            <div className="space-y-2 mt-4 text-xs text-slate-300">
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-500">Service Version</span>
                <span className="font-mono text-slate-300">v13 (IPC 0)</span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-500">Shell UID</span>
                <span className="font-mono text-slate-300">2000 (Shell)</span>
              </div>
              <div className="flex justify-between py-1">
                <span className="text-slate-500">Permission Hook</span>
                <span className="font-mono text-emerald-400">Active</span>
              </div>
            </div>
          </div>
          <div className="mt-4 pt-3 border-t border-slate-800/60 text-[11px] text-slate-500">
            Lifecycle & Keyguard Dismiss (BB-R7)
          </div>
        </div>

        {/* Card 2: Reverse WebSocket Tunnel */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 flex flex-col justify-between shadow-lg">
          <div>
            <div className="flex items-center justify-between mb-3">
              <span className="text-xs font-medium text-slate-400">Reverse WS Tunnel</span>
              {renderTunnelBadge(state.tunnelState)}
            </div>
            <div className="space-y-2 mt-4 text-xs text-slate-300">
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-500">Endpoint</span>
                <span className="font-mono text-slate-300 truncate max-w-[140px]" title={state.pairing?.url || 'wss://desktop.lan/tunnel'}>
                  {state.pairing?.host || 'desktop.lan'}
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-500">Round-trip Ping</span>
                <span className="font-mono text-emerald-400">
                  {state.tunnelState === 'CONNECTED' ? `${state.vitals.latencyMs} ms` : '—'}
                </span>
              </div>
              <div className="flex justify-between py-1">
                <span className="text-slate-500">Auth Token</span>
                <span className="font-mono text-slate-300">
                  {state.pairing?.token ? 'Bearer Verified' : 'Unpaired'}
                </span>
              </div>
            </div>
          </div>
          <div className="mt-4 pt-3 border-t border-slate-800/60 flex items-center justify-between text-[11px]">
            <span className="text-slate-500">Supervisor: Active</span>
            <button
              onClick={() => reconnectTunnel()}
              className="text-indigo-400 hover:text-indigo-300 font-medium"
            >
              Reconnect
            </button>
          </div>
        </div>

        {/* Card 3: Target App State */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 flex flex-col justify-between shadow-lg">
          <div>
            <div className="flex items-center justify-between mb-3">
              <span className="text-xs font-medium text-slate-400">Target Application</span>
              {renderAppStatusBadge(state.appStatus)}
            </div>
            <div className="space-y-2 mt-4 text-xs text-slate-300">
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-500">Package</span>
                <span className="font-mono text-slate-300 truncate max-w-[140px]" title="com.example.target">
                  com.example.target
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-500">Activity</span>
                <span className="font-mono text-slate-300">.MainActivity</span>
              </div>
              <div className="flex justify-between py-1">
                <span className="text-slate-500">Bridge Link</span>
                <span className="font-mono text-emerald-400">
                  {state.appStatus === 'FOREGROUND' ? '127.0.0.1:8765' : 'Offline'}
                </span>
              </div>
            </div>
          </div>
          <div className="mt-4 pt-3 border-t border-slate-800/60 text-[11px] text-slate-500">
            Process Automation (BB-R8)
          </div>
        </div>

        {/* Card 4: Device Vitals & Telemetry */}
        <div className="bg-slate-900/80 border border-slate-800 rounded-xl p-5 flex flex-col justify-between shadow-lg">
          <div>
            <div className="flex items-center justify-between mb-3">
              <span className="text-xs font-medium text-slate-400">Device Vitals</span>
              <span className="text-xs font-mono text-slate-300 flex items-center space-x-1">
                <Smartphone className="w-3.5 h-3.5 text-slate-400" />
                <span>Pixel 8</span>
              </span>
            </div>
            <div className="space-y-2 mt-4 text-xs text-slate-300">
              <div className="flex justify-between items-center py-1 border-b border-slate-800/60">
                <span className="text-slate-500 flex items-center space-x-1.5">
                  {renderBatteryIcon(state.vitals.batteryPercent)}
                  <span>Battery</span>
                </span>
                <div className="flex items-center space-x-2">
                  <div className="w-16 h-2 bg-slate-800 rounded-full overflow-hidden">
                    <div
                      className={`h-full rounded-full transition-all duration-300 ${
                        state.vitals.batteryPercent <= 20
                          ? 'bg-rose-500'
                          : state.vitals.batteryPercent <= 50
                          ? 'bg-amber-500'
                          : 'bg-emerald-500'
                      }`}
                      style={{ width: `${state.vitals.batteryPercent}%` }}
                    />
                  </div>
                  <span className="font-mono text-slate-200">{state.vitals.batteryPercent}%</span>
                </div>
              </div>

              <div className="flex justify-between items-center py-1 border-b border-slate-800/60">
                <span className="text-slate-500 flex items-center space-x-1.5">
                  {state.vitals.screenOn ? (
                    <Eye className="w-4 h-4 text-emerald-400" />
                  ) : (
                    <EyeOff className="w-4 h-4 text-rose-400" />
                  )}
                  <span>Screen State</span>
                </span>
                <span
                  data-testid="screen-state-indicator"
                  className={`font-semibold font-mono text-xs ${
                    state.vitals.screenOn ? 'text-emerald-400' : 'text-rose-400'
                  }`}
                >
                  {state.vitals.screenOn ? 'ON' : 'OFF'}
                </span>
              </div>

              <div className="flex justify-between items-center py-1">
                <span className="text-slate-500 flex items-center space-x-1.5">
                  {renderNetworkIcon(state.vitals.networkType)}
                  <span>Transport</span>
                </span>
                <span className="font-mono text-slate-300">{state.vitals.networkType}</span>
              </div>
            </div>
          </div>
          <div className="mt-4 pt-3 border-t border-slate-800/60 text-[11px] text-slate-500">
            Hardware Telemetry
          </div>
        </div>
      </div>

      {/* Expanded Infrastructure Summary */}
      <div className="bg-slate-900/60 border border-slate-800 rounded-xl p-5 shadow-lg">
        <h3 className="text-sm font-semibold text-slate-200 mb-3 flex items-center space-x-2">
          <Server className="w-4 h-4 text-indigo-400" />
          <span>Active Tunnel Topology (Map 10 Remote Architecture)</span>
        </h3>
        <div className="flex flex-col sm:flex-row items-center justify-between text-xs text-slate-400 gap-4 pt-2">
          <div className="flex items-center space-x-2 bg-slate-950 px-3 py-2 rounded-lg border border-slate-800 w-full sm:w-auto">
            <Smartphone className="w-4 h-4 text-indigo-400 shrink-0" />
            <div>
              <div className="font-medium text-slate-200">Android Host Device</div>
              <div className="text-[11px] text-slate-500 font-mono">192.168.1.105 (LAN Direct)</div>
            </div>
          </div>

          <div className="flex items-center space-x-1 text-slate-600 font-mono">
            <span>───</span>
            <span className="text-indigo-400 font-bold">wss://</span>
            <span>───►</span>
          </div>

          <div className="flex items-center space-x-2 bg-slate-950 px-3 py-2 rounded-lg border border-slate-800 w-full sm:w-auto">
            <Radio className="w-4 h-4 text-emerald-400 shrink-0" />
            <div>
              <div className="font-medium text-slate-200">Desktop MCP Server</div>
              <div className="text-[11px] text-slate-500 font-mono">localhost:8765 (Gateway)</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
