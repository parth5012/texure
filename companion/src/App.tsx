import React, { useState } from 'react';
import { CompanionProvider, useCompanion } from './context/CompanionContext';
import { MockToolbar } from './components/MockToolbar';
import { QrScanner } from './components/QrScanner';
import { Smartphone, QrCode, Sliders, Terminal, Activity, Moon, Sun } from 'lucide-react';

const DashboardShell: React.FC = () => {
  const { state } = useCompanion();
  const [activeTab, setActiveTab] = useState<'dashboard' | 'controls' | 'scanner' | 'logs'>('dashboard');
  const [darkMode, setDarkMode] = useState(true);

  return (
    <div className={`min-h-screen ${darkMode ? 'dark bg-slate-950 text-slate-100' : 'bg-slate-50 text-slate-900'} flex flex-col font-sans transition-colors duration-150`}>
      <MockToolbar />

      {/* Top Header */}
      <header className="border-b border-slate-800 bg-slate-900/60 backdrop-blur px-6 py-3 flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <div className="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white font-bold shadow-lg shadow-indigo-500/30">
            <Smartphone className="w-5 h-5" />
          </div>
          <div>
            <h1 className="text-base font-semibold tracking-wide">Texure Companion</h1>
            <p className="text-xs text-slate-400">Android Test Bridge & Remote Gateway</p>
          </div>
        </div>

        <div className="flex items-center space-x-4">
          <div className="flex items-center space-x-2 text-xs px-2.5 py-1 rounded-full bg-slate-800 border border-slate-700">
            <span
              className={`w-2 h-2 rounded-full ${
                state.tunnelState === 'CONNECTED'
                  ? 'bg-emerald-400 animate-pulse'
                  : state.tunnelState === 'CONNECTING'
                  ? 'bg-amber-400 animate-ping'
                  : 'bg-rose-400'
              }`}
            />
            <span className="font-mono text-slate-300">
              {state.tunnelState === 'CONNECTED' ? 'Tunnel Online' : state.tunnelState}
            </span>
            {state.tunnelState === 'CONNECTED' && (
              <span className="text-slate-400 border-l border-slate-700 pl-2">
                {state.vitals.latencyMs}ms
              </span>
            )}
          </div>

          <button
            onClick={() => setDarkMode(!darkMode)}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-200 hover:bg-slate-800 transition"
            title="Toggle theme"
          >
            {darkMode ? <Sun className="w-4 h-4" /> : <Moon className="w-4 h-4" />}
          </button>
        </div>
      </header>

      {/* Navigation Bar */}
      <div className="border-b border-slate-800 bg-slate-900/40 px-6">
        <nav className="flex space-x-6 text-sm">
          <button
            onClick={() => setActiveTab('dashboard')}
            className={`py-3 px-1 border-b-2 font-medium flex items-center space-x-2 transition ${
              activeTab === 'dashboard'
                ? 'border-indigo-500 text-indigo-400'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Activity className="w-4 h-4" />
            <span>Device Status</span>
          </button>

          <button
            onClick={() => setActiveTab('controls')}
            className={`py-3 px-1 border-b-2 font-medium flex items-center space-x-2 transition ${
              activeTab === 'controls'
                ? 'border-indigo-500 text-indigo-400'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Sliders className="w-4 h-4" />
            <span>Manual Controls</span>
          </button>

          <button
            onClick={() => setActiveTab('scanner')}
            className={`py-3 px-1 border-b-2 font-medium flex items-center space-x-2 transition ${
              activeTab === 'scanner'
                ? 'border-indigo-500 text-indigo-400'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <QrCode className="w-4 h-4" />
            <span>QR Pairing</span>
          </button>

          <button
            onClick={() => setActiveTab('logs')}
            className={`py-3 px-1 border-b-2 font-medium flex items-center space-x-2 transition ${
              activeTab === 'logs'
                ? 'border-indigo-500 text-indigo-400'
                : 'border-transparent text-slate-400 hover:text-slate-200'
            }`}
          >
            <Terminal className="w-4 h-4" />
            <span>Live Logs</span>
          </button>
        </nav>
      </div>

      {/* Main Content Area */}
      <main className="flex-1 p-6 max-w-7xl w-full mx-auto">
        <div id="tab-content" className="space-y-6">
          {activeTab === 'dashboard' && (
            <div data-testid="dashboard-view" className="p-6 bg-slate-900/60 rounded-xl border border-slate-800">
              <h2 className="text-lg font-semibold text-slate-100 mb-2">Device & Shizuku Status</h2>
              <p className="text-sm text-slate-400">
                Connected tunnel state: <strong className="text-emerald-400">{state.tunnelState}</strong> | Shizuku: <strong className="text-indigo-400">{state.shizukuStatus}</strong>
              </p>
            </div>
          )}

          {activeTab === 'controls' && (
            <div data-testid="controls-view" className="p-6 bg-slate-900/60 rounded-xl border border-slate-800">
              <h2 className="text-lg font-semibold text-slate-100 mb-2">Device Control Panel</h2>
              <p className="text-sm text-slate-400">Manual lifecycle, wake, and keyguard controls.</p>
            </div>
          )}

          {activeTab === 'scanner' && (
            <div data-testid="scanner-view">
              <QrScanner />
            </div>
          )}

          {activeTab === 'logs' && (
            <div data-testid="logs-view" className="p-6 bg-slate-900/60 rounded-xl border border-slate-800">
              <h2 className="text-lg font-semibold text-slate-100 mb-2">Real-time Event Stream</h2>
              <p className="text-sm text-slate-400">Streaming logs and lifecycle events.</p>
            </div>
          )}
        </div>
      </main>

      {/* Footer */}
      <footer className="border-t border-slate-800 px-6 py-3 text-xs text-slate-500 flex justify-between items-center bg-slate-950">
        <div>Texure Companion Shell v0.1.0 • Tauri v2</div>
        <div className="font-mono">LAN Direct & Reverse WS Gateway</div>
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
