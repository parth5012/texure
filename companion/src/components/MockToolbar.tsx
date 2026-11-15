import React, { useState } from 'react';
import { useCompanion } from '../context/CompanionContext';
import { ShizukuStatus, TunnelState, AppStatus } from '../types';
import { Wrench, ChevronDown, ChevronUp } from 'lucide-react';

export const MockToolbar: React.FC = () => {
  const { state, updateMockState, addLog, isBrowserMock } = useCompanion();
  const [open, setOpen] = useState(false);

  if (!isBrowserMock) return null;

  return (
    <div className="bg-slate-800/90 backdrop-blur border-b border-indigo-500/30 text-xs px-4 py-2 sticky top-0 z-50 transition-all">
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-2 text-indigo-400 font-medium">
          <Wrench className="w-3.5 h-3.5" />
          <span>Browser Mock Mode Active</span>
        </div>
        <button
          onClick={() => setOpen(!open)}
          className="text-slate-400 hover:text-slate-200 flex items-center space-x-1"
        >
          <span>{open ? 'Hide Simulator Bar' : 'Toggle Simulator Controls'}</span>
          {open ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
        </button>
      </div>

      {open && (
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mt-3 pt-3 border-t border-slate-700">
          <div>
            <label className="block text-slate-400 mb-1">Shizuku Status</label>
            <select
              value={state.shizukuStatus}
              onChange={(e) => updateMockState({ shizukuStatus: e.target.value as ShizukuStatus })}
              className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-slate-200 w-full"
            >
              <option value="AUTHORIZED">AUTHORIZED</option>
              <option value="UNAUTHORIZED">UNAUTHORIZED</option>
              <option value="SERVICE_DEAD">SERVICE_DEAD</option>
            </select>
          </div>

          <div>
            <label className="block text-slate-400 mb-1">Tunnel State</label>
            <select
              value={state.tunnelState}
              onChange={(e) => updateMockState({ tunnelState: e.target.value as TunnelState })}
              className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-slate-200 w-full"
            >
              <option value="CONNECTED">CONNECTED</option>
              <option value="CONNECTING">CONNECTING</option>
              <option value="DISCONNECTED">DISCONNECTED</option>
              <option value="ERROR">ERROR</option>
            </select>
          </div>

          <div>
            <label className="block text-slate-400 mb-1">Target App Status</label>
            <select
              value={state.appStatus}
              onChange={(e) => updateMockState({ appStatus: e.target.value as AppStatus })}
              className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-slate-200 w-full"
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
              className={`px-3 py-1 rounded border text-xs w-full ${
                state.vitals.screenOn
                  ? 'bg-emerald-950 border-emerald-600 text-emerald-300'
                  : 'bg-rose-950 border-rose-600 text-rose-300'
              }`}
            >
              Screen: {state.vitals.screenOn ? 'ON' : 'OFF'}
            </button>
            <button
              onClick={() =>
                addLog('INFO', `Simulated tick event at ${new Date().toLocaleTimeString()}`, 'mock')
              }
              className="px-3 py-1 rounded bg-slate-700 hover:bg-slate-600 border border-slate-600 text-slate-200 text-xs whitespace-nowrap"
            >
              + Log
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
