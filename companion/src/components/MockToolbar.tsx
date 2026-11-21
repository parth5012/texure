import React, { useState } from 'react';
import { useCompanion } from '../context/CompanionContext';
import { ShizukuStatus, TunnelState, AppStatus } from '../types';
import { Wrench, ChevronDown, ChevronUp } from 'lucide-react';

export const MockToolbar: React.FC = () => {
  const { state, updateMockState, addLog, isBrowserMock } = useCompanion();
  const [open, setOpen] = useState(false);

  if (!isBrowserMock) return null;

  return (
    <div data-testid="mock-toolbar" className="bg-amber-50/70 dark:bg-zinc-900/90 backdrop-blur-md border-b border-amber-200/60 dark:border-amber-500/20 text-xs px-6 py-2 sticky top-0 z-50 transition-colors shadow-xs">
      <div className="flex items-center justify-between max-w-7xl mx-auto">
        <div className="flex items-center space-x-2.5 text-amber-900 dark:text-amber-400 font-medium">
          <div className="w-5 h-5 rounded-md bg-amber-500/15 border border-amber-500/30 flex items-center justify-center shrink-0">
            <Wrench className="w-3 h-3 text-amber-600 dark:text-amber-400" />
          </div>
          <span className="font-semibold text-xs tracking-wide">Browser Mock Mode Active</span>
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/10 dark:bg-amber-500/20 text-amber-800 dark:text-amber-300 border border-amber-200/80 dark:border-amber-600/40 font-mono font-medium">
            SIMULATION
          </span>
        </div>
        <button
          onClick={() => setOpen(!open)}
          className="text-amber-800 dark:text-zinc-400 hover:text-amber-950 dark:hover:text-zinc-100 flex items-center space-x-1.5 px-2.5 py-1 rounded-lg hover:bg-amber-100/60 dark:hover:bg-zinc-800 transition text-xs font-medium"
        >
          <span>{open ? 'Hide Simulator Bar' : 'Toggle Simulator Controls'}</span>
          {open ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
        </button>
      </div>

      {open && (
        <div className="max-w-7xl mx-auto grid grid-cols-2 md:grid-cols-4 gap-3.5 mt-3 pt-3 border-t border-amber-200/80 dark:border-zinc-800">
          <div>
            <label className="block text-amber-900 dark:text-zinc-400 mb-1 font-medium text-xs">Shizuku Status</label>
            <select
              value={state.shizukuStatus}
              onChange={(e) => updateMockState({ shizukuStatus: e.target.value as ShizukuStatus })}
              className="bg-white dark:bg-zinc-950 border border-amber-300 dark:border-zinc-700 rounded-xl px-2.5 py-1.5 text-slate-800 dark:text-zinc-200 w-full focus:outline-none focus:border-indigo-500 text-xs font-mono"
            >
              <option value="AUTHORIZED">AUTHORIZED</option>
              <option value="UNAUTHORIZED">UNAUTHORIZED</option>
              <option value="SERVICE_DEAD">SERVICE_DEAD</option>
            </select>
          </div>

          <div>
            <label className="block text-amber-900 dark:text-zinc-400 mb-1 font-medium text-xs">Tunnel State</label>
            <select
              value={state.tunnelState}
              onChange={(e) => updateMockState({ tunnelState: e.target.value as TunnelState })}
              className="bg-white dark:bg-zinc-950 border border-amber-300 dark:border-zinc-700 rounded-xl px-2.5 py-1.5 text-slate-800 dark:text-zinc-200 w-full focus:outline-none focus:border-indigo-500 text-xs font-mono"
            >
              <option value="CONNECTED">CONNECTED</option>
              <option value="CONNECTING">CONNECTING</option>
              <option value="DISCONNECTED">DISCONNECTED</option>
              <option value="ERROR">ERROR</option>
            </select>
          </div>

          <div>
            <label className="block text-amber-900 dark:text-zinc-400 mb-1 font-medium text-xs">Target App Status</label>
            <select
              value={state.appStatus}
              onChange={(e) => updateMockState({ appStatus: e.target.value as AppStatus })}
              className="bg-white dark:bg-zinc-950 border border-amber-300 dark:border-zinc-700 rounded-xl px-2.5 py-1.5 text-slate-800 dark:text-zinc-200 w-full focus:outline-none focus:border-indigo-500 text-xs font-mono"
            >
              <option value="FOREGROUND">FOREGROUND</option>
              <option value="BACKGROUND">BACKGROUND</option>
              <option value="STOPPED">STOPPED</option>
            </select>
          </div>

          <div className="flex items-end space-x-2">
            <button
              onClick={() =>
                updateMockState({
                  vitals: {
                    ...state.vitals,
                    screenOn: !state.vitals.screenOn,
                  },
                })
              }
              className={`px-3 py-1.5 rounded-xl border text-xs font-medium w-full transition shadow-xs ${
                state.vitals.screenOn
                  ? 'bg-emerald-600 dark:bg-emerald-950/80 border-emerald-500 dark:border-emerald-600/80 text-white dark:text-emerald-300'
                  : 'bg-rose-600 dark:bg-rose-950/80 border-rose-500 dark:border-rose-600/80 text-white dark:text-rose-300'
              }`}
            >
              Screen: {state.vitals.screenOn ? 'ON' : 'OFF'}
            </button>
            <button
              onClick={() =>
                addLog('INFO', `Simulated tick event at ${new Date().toLocaleTimeString()}`, 'mock')
              }
              className="px-3 py-1.5 rounded-xl bg-white hover:bg-slate-50 dark:bg-zinc-800 dark:hover:bg-zinc-700 border border-amber-300 dark:border-zinc-700 text-slate-800 dark:text-zinc-200 text-xs font-medium whitespace-nowrap transition shadow-xs"
            >
              + Log
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
