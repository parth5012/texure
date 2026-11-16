import React, { createContext, useContext, useEffect, useState, useCallback } from 'react';
import { DeviceState, LogEntry, PairingConfig, LogLevel } from '../types';
import { invokeCommand, listenEvent, isTauri } from '../lib/tauri-bridge';
import { mockBackend } from '../lib/mock-backend';

interface CompanionContextType {
  state: DeviceState;
  logs: LogEntry[];
  isBrowserMock: boolean;
  invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
  refreshState: () => Promise<void>;
  pairDevice: (config: PairingConfig) => Promise<boolean>;
  wakeScreen: () => Promise<void>;
  dismissKeyguard: () => Promise<void>;
  coldStartApp: (pkg?: string) => Promise<void>;
  forceStopApp: (pkg?: string) => Promise<void>;
  reconnectTunnel: () => Promise<void>;
  clearLogs: () => Promise<void>;
  exportLogs: () => Promise<string>;
  updateMockState: (patch: Partial<DeviceState>) => void;
  addLog: (level: LogLevel, message: string, source?: string) => void;
}

const normalizeState = (s: DeviceState): DeviceState => ({
  ...s,
  vitals: {
    batteryPercent: s?.vitals?.batteryPercent ?? 100,
    screenOn: s?.vitals?.screenOn ?? true,
    networkType: s?.vitals?.networkType ?? 'WIFI',
    latencyMs: s?.vitals?.latencyMs ?? 0,
  },
});

const defaultState: DeviceState = {
  shizukuStatus: 'AUTHORIZED',
  tunnelState: 'CONNECTED',
  appStatus: 'FOREGROUND',
  vitals: {
    batteryPercent: 88,
    screenOn: true,
    networkType: 'WIFI',
    latencyMs: 14,
  },
  pairing: null,
  paired: false,
};

const CompanionContext = createContext<CompanionContextType | null>(null);

export const CompanionProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, setState] = useState<DeviceState>(defaultState);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const isBrowserMock = !isTauri();

  const refreshState = useCallback(async () => {
    try {
      const devState = await invokeCommand<DeviceState>('get_device_state');
      setState(normalizeState(devState));
    } catch (err) {
      console.error('Failed to get device state:', err);
    }
  }, []);

  const refreshLogs = useCallback(async () => {
    try {
      const logEntries = await invokeCommand<LogEntry[]>('get_logs');
      setLogs(logEntries);
    } catch (err) {
      console.error('Failed to get logs:', err);
    }
  }, []);

  useEffect(() => {
    refreshState();
    refreshLogs();

    const unlistenState = listenEvent<DeviceState>('device_state_changed', (updatedState) => {
      setState(normalizeState(updatedState));
    });

    const unlistenLog = listenEvent<LogEntry>('tauri://event/log', (entry) => {
      setLogs((prev) => [...prev.slice(-499), entry]);
    });

    const unlistenClear = listenEvent('logs_cleared', () => {
      setLogs([]);
    });

    return () => {
      unlistenState();
      unlistenLog();
      unlistenClear();
    };
  }, [refreshState, refreshLogs]);

  const pairDevice = useCallback(async (config: PairingConfig) => {
    try {
      await invokeCommand('pair_device', { config });
      await refreshState();
      return true;
    } catch (err) {
      console.error('Pairing failed:', err);
      return false;
    }
  }, [refreshState]);

  const wakeScreen = useCallback(async () => {
    await invokeCommand('wake_screen');
    await refreshState();
  }, [refreshState]);

  const dismissKeyguard = useCallback(async () => {
    await invokeCommand('dismiss_keyguard');
    await refreshState();
  }, [refreshState]);

  const coldStartApp = useCallback(async (pkg = 'com.example.target') => {
    await invokeCommand('cold_start_app', { package: pkg });
    await refreshState();
  }, [refreshState]);

  const forceStopApp = useCallback(async (pkg = 'com.example.target') => {
    await invokeCommand('force_stop_app', { package: pkg });
    await refreshState();
  }, [refreshState]);

  const reconnectTunnel = useCallback(async () => {
    await invokeCommand('reconnect_tunnel');
    await refreshState();
  }, [refreshState]);

  const clearLogs = useCallback(async () => {
    await invokeCommand('clear_logs');
    setLogs([]);
  }, []);

  const exportLogs = useCallback(async () => {
    return await invokeCommand<string>('export_logs');
  }, []);

  const updateMockState = useCallback((patch: Partial<DeviceState>) => {
    mockBackend.invoke('set_mock_state', { patch });
  }, []);

  const addLog = useCallback((level: LogLevel, message: string, source?: string) => {
    mockBackend.addLog(level, message, source);
  }, []);

  const contextValue = React.useMemo<CompanionContextType>(
    () => ({
      state,
      logs,
      isBrowserMock,
      invoke: invokeCommand,
      refreshState,
      pairDevice,
      wakeScreen,
      dismissKeyguard,
      coldStartApp,
      forceStopApp,
      reconnectTunnel,
      clearLogs,
      exportLogs,
      updateMockState,
      addLog,
    }),
    [
      state,
      logs,
      isBrowserMock,
      refreshState,
      pairDevice,
      wakeScreen,
      dismissKeyguard,
      coldStartApp,
      forceStopApp,
      reconnectTunnel,
      clearLogs,
      exportLogs,
      updateMockState,
      addLog,
    ]
  );

  return (
    <CompanionContext.Provider value={contextValue}>
      {children}
    </CompanionContext.Provider>
  );
};

export const useCompanion = () => {
  const context = useContext(CompanionContext);
  if (!context) {
    throw new Error('useCompanion must be used within a CompanionProvider');
  }
  return context;
};
