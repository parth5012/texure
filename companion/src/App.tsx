import React, { useState, useEffect } from 'react';
import { CompanionProvider, useCompanion } from './context/CompanionContext';
import { MockToolbar } from './components/MockToolbar';
import { QrScanner } from './components/QrScanner';
import { StatusDashboard } from './components/StatusDashboard';
import { ControlPanel } from './components/ControlPanel';
import { LogViewer } from './components/LogViewer';
import { ErrorBoundary } from './components/ErrorBoundary';
import { Smartphone, QrCode, Sliders, Terminal, Activity, Moon, Sun, Wifi } from 'lucide-react';

const getInitialTheme = (): boolean => {
  try {
    if (typeof window !== 'undefined' && 'localStorage' in window && window.localStorage) {
      const saved = window.localStorage.getItem('texure_theme');
      if (saved !== null) {
        return saved === 'dark';
      }
      if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
        return true;
      }
    }
  } catch {
    // fallback
  }
  return true;
};

const DashboardShell: React.FC = () => {
  const { state } = useCompanion();
  const [activeTab, setActiveTab] = useState<'dashboard' | 'controls' | 'scanner' | 'logs'>('dashboard');
  const [darkMode, setDarkMode] = useState<boolean>(getInitialTheme);

  useEffect(() => {
    if (typeof document !== 'undefined') {
      const root = document.documentElement;
      if (darkMode) {
        root.classList.add('dark');
        root.style.colorScheme = 'dark';
      } else {
        root.classList.remove('dark');
        root.style.colorScheme = 'light';
      }
      try {
        localStorage.setItem('texure_theme', darkMode ? 'dark' : 'light');
      } catch {
        // ignore
      }
    }
  }, [darkMode]);

  return (
    <div
      className={`min-h-screen ${
        darkMode ? 'dark bg-[#09090b] text-zinc-100' : 'bg-slate-100 text-slate-900'
      } flex flex-col font-sans transition-colors duration-200 relative selection:bg-indigo-500/20 selection:text-indigo-600 dark:selection:text-indigo-300`}
    >
      {/* Ambient background glow for visual depth */}
      <div
        className="pointer-events-none fixed inset-0 z-0 overflow-hidden"
        aria-hidden="true"
      >
        <div
          className={`absolute -top-32 left-1/2 -translate-x-1/2 w-[1000px] h-[400px] rounded-full blur-3xl transition-opacity duration-300 ${
            darkMode
              ? 'bg-gradient-to-r from-indigo-600/25 via-violet-600/20 to-sky-500/15 opacity-40'
              : 'bg-gradient-to-r from-indigo-100/40 via-sky-100/30 to-slate-100/20 opacity-30'
          }`}
        />
        <div
          className={`absolute top-48 left-1/2 -translate-x-1/2 w-[1200px] h-[450px] rounded-full blur-[90px] pointer-events-none ${
            darkMode
              ? 'bg-gradient-to-b from-indigo-950/20 via-zinc-900/10 to-transparent opacity-60'
              : 'bg-gradient-to-b from-slate-200/50 via-slate-100/20 to-transparent opacity-50'
          }`}
        />
      </div>

      <MockToolbar />

      {/* Main App Bar / Header */}
      <header className="relative z-40 border-b border-slate-200/80 dark:border-zinc-800/80 bg-white/80 dark:bg-[#09090b]/80 backdrop-blur-xl px-6 py-3.5 flex items-center justify-between transition-colors shadow-xs">
        <div className="flex items-center space-x-3.5">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-indigo-500 via-indigo-600 to-violet-700 flex items-center justify-center text-white shadow-md shadow-indigo-500/25 ring-1 ring-white/20">
            <Smartphone className="w-5 h-5 drop-shadow-sm" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <h1 className="text-sm font-semibold tracking-tight text-slate-900 dark:text-white">
                Texure Companion
              </h1>
              <span className="px-1.5 py-0.5 rounded-md text-[10px] font-mono font-medium tracking-wide bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/40">
                GATEWAY
              </span>
            </div>
            <p className="text-xs text-slate-500 dark:text-zinc-400 font-normal">
              Android Test Bridge &amp; Remote Gateway
            </p>
          </div>
        </div>

        <div className="flex items-center space-x-3">
          {/* Tunnel status indicator pill */}
          <div className="flex items-center space-x-2 text-xs px-3 py-1.5 rounded-full bg-slate-100 dark:bg-zinc-900/90 border border-slate-200/80 dark:border-zinc-800/80 shadow-xs">
            <span className="relative flex h-2 w-2">
              {state.tunnelState === 'CONNECTED' ? (
                <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500 shadow-xs shadow-emerald-500/50" />
              ) : state.tunnelState === 'CONNECTING' ? (
                <span className="relative inline-flex rounded-full h-2 w-2 bg-amber-500 shadow-xs shadow-amber-500/50 animate-pulse" />
              ) : (
                <span className="relative inline-flex rounded-full h-2 w-2 bg-rose-500" />
              )}
            </span>
            <span className="font-mono text-slate-700 dark:text-zinc-300 font-medium text-xs">
              {state.tunnelState === 'CONNECTED' ? 'Tunnel Online' : state.tunnelState}
            </span>
            {state.tunnelState === 'CONNECTED' && (
              <span className="flex items-center space-x-1 font-mono text-emerald-700 dark:text-emerald-400 border-l border-slate-200 dark:border-zinc-800 pl-2 text-xs font-medium">
                <Wifi className="w-3 h-3 opacity-70" />
                <span>{state.vitals.latencyMs} ms</span>
              </span>
            )}
          </div>

          {/* Theme switcher */}
          <button
            onClick={() => setDarkMode(!darkMode)}
            className="p-2 rounded-xl text-slate-500 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-100 hover:bg-slate-100 dark:hover:bg-zinc-800/80 border border-slate-200/80 dark:border-zinc-800/80 transition-all shadow-xs"
            title="Toggle theme"
            aria-label="Toggle theme"
            aria-pressed={darkMode}
          >
            {darkMode ? <Sun className="w-4 h-4 text-amber-400" /> : <Moon className="w-4 h-4 text-slate-600" />}
          </button>
        </div>
      </header>

      {/* Modern Segmented Navigation Bar */}
      <div className="relative z-30 border-b border-slate-200/80 dark:border-zinc-800/60 bg-white/60 dark:bg-[#09090b]/50 backdrop-blur-md px-6 py-2.5">
        <nav className="inline-flex p-1 bg-slate-100/90 dark:bg-zinc-900/90 rounded-xl border border-slate-200/70 dark:border-zinc-800/70 space-x-1 text-xs font-medium">
          <button
            onClick={() => setActiveTab('dashboard')}
            aria-current={activeTab === 'dashboard' ? 'page' : undefined}
            className={`py-2 px-3.5 rounded-lg flex items-center space-x-2 transition-all duration-150 ${
              activeTab === 'dashboard'
                ? 'bg-white dark:bg-zinc-800 text-slate-900 dark:text-white shadow-xs font-semibold'
                : 'text-slate-600 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200 hover:bg-white/60 dark:hover:bg-zinc-800/50'
            }`}
          >
            <Activity className={`w-3.5 h-3.5 ${activeTab === 'dashboard' ? 'text-indigo-500 dark:text-indigo-400' : ''}`} />
            <span>Device Status</span>
          </button>

          <button
            onClick={() => setActiveTab('controls')}
            aria-current={activeTab === 'controls' ? 'page' : undefined}
            className={`py-2 px-3.5 rounded-lg flex items-center space-x-2 transition-all duration-150 ${
              activeTab === 'controls'
                ? 'bg-white dark:bg-zinc-800 text-slate-900 dark:text-white shadow-xs font-semibold'
                : 'text-slate-600 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200 hover:bg-white/60 dark:hover:bg-zinc-800/50'
            }`}
          >
            <Sliders className={`w-3.5 h-3.5 ${activeTab === 'controls' ? 'text-indigo-500 dark:text-indigo-400' : ''}`} />
            <span>Manual Controls</span>
          </button>

          <button
            onClick={() => setActiveTab('scanner')}
            aria-current={activeTab === 'scanner' ? 'page' : undefined}
            className={`py-2 px-3.5 rounded-lg flex items-center space-x-2 transition-all duration-150 ${
              activeTab === 'scanner'
                ? 'bg-white dark:bg-zinc-800 text-slate-900 dark:text-white shadow-xs font-semibold'
                : 'text-slate-600 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200 hover:bg-white/60 dark:hover:bg-zinc-800/50'
            }`}
          >
            <QrCode className={`w-3.5 h-3.5 ${activeTab === 'scanner' ? 'text-indigo-500 dark:text-indigo-400' : ''}`} />
            <span>QR Pairing</span>
          </button>

          <button
            onClick={() => setActiveTab('logs')}
            aria-current={activeTab === 'logs' ? 'page' : undefined}
            className={`py-2 px-3.5 rounded-lg flex items-center space-x-2 transition-all duration-150 ${
              activeTab === 'logs'
                ? 'bg-white dark:bg-zinc-800 text-slate-900 dark:text-white shadow-xs font-semibold'
                : 'text-slate-600 dark:text-zinc-400 hover:text-slate-800 dark:hover:text-zinc-200 hover:bg-white/60 dark:hover:bg-zinc-800/50'
            }`}
          >
            <Terminal className={`w-3.5 h-3.5 ${activeTab === 'logs' ? 'text-indigo-500 dark:text-indigo-400' : ''}`} />
            <span>Live Logs</span>
          </button>
        </nav>
      </div>

      {/* Main Content Area */}
      <main className="flex-1 p-6 max-w-7xl w-full mx-auto relative z-10">
        <ErrorBoundary key={activeTab} fallbackTitle={`Error rendering ${activeTab} tab`}>
          <div id="tab-content" className="space-y-6">
            {activeTab === 'dashboard' && (
              <div data-testid="dashboard-view" className="animate-tab-fade">
                <StatusDashboard />
              </div>
            )}

            {activeTab === 'controls' && (
              <div data-testid="controls-view" className="animate-tab-fade">
                <ControlPanel />
              </div>
            )}

            {activeTab === 'scanner' && (
              <div data-testid="scanner-view" className="animate-tab-fade">
                <QrScanner />
              </div>
            )}

            {activeTab === 'logs' && (
              <div data-testid="logs-view" className="animate-tab-fade">
                <LogViewer />
              </div>
            )}
          </div>
        </ErrorBoundary>
      </main>

      {/* Polished Quiet Footer */}
      <footer className="relative z-10 border-t border-slate-200/80 dark:border-zinc-800/80 px-6 py-3.5 text-xs text-slate-600 dark:text-zinc-400 flex justify-between items-center bg-white/60 dark:bg-[#09090b]/60 backdrop-blur-md">
        <div className="flex items-center space-x-2">
          <span>Texure Companion Shell v0.1.0</span>
          <span>•</span>
          <span className="font-mono text-xs text-slate-600 dark:text-zinc-400">Tauri v2 + React 18</span>
        </div>
        <div className="flex items-center space-x-1.5 font-mono text-xs text-slate-600 dark:text-zinc-400">
          <span className={`w-1.5 h-1.5 rounded-full ${state.tunnelState === 'CONNECTED' ? 'bg-emerald-500' : 'bg-slate-400'}`} />
          <span>{state.tunnelState === 'CONNECTED' ? 'LAN Direct & Reverse WS Gateway' : 'Gateway Standby'}</span>
        </div>
      </footer>
    </div>
  );
};

export function App() {
  return (
    <CompanionProvider>
      <DashboardShell />
    </CompanionProvider>
  );
}

export default App;
