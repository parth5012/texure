export type ShizukuStatus = 'AUTHORIZED' | 'UNAUTHORIZED' | 'SERVICE_DEAD';
export type TunnelState = 'CONNECTED' | 'CONNECTING' | 'DISCONNECTED' | 'ERROR';
export type AppStatus = 'FOREGROUND' | 'BACKGROUND' | 'STOPPED';
export type NetworkType = 'WIFI' | 'CELLULAR' | 'NONE';

export interface DeviceVitals {
  batteryPercent: number;
  screenOn: boolean;
  networkType: NetworkType;
  latencyMs: number;
}

export interface PairingConfig {
  scheme: string;
  host: string;
  port: number;
  path: string;
  token: string;
  app?: string;
  url?: string;
}

export type LogLevel = 'INFO' | 'WARN' | 'ERROR' | 'DEBUG';

export interface LogEntry {
  id: string;
  timestamp: string;
  level: LogLevel;
  message: string;
  source?: string;
}

export interface DeviceState {
  shizukuStatus: ShizukuStatus;
  tunnelState: TunnelState;
  appStatus: AppStatus;
  vitals: DeviceVitals;
  pairing: PairingConfig | null;
  paired: boolean;
}
