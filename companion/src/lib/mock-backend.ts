import { DeviceState, LogEntry, PairingConfig } from '../types';

type Listener<T> = (payload: T) => void;

class MockBackend {
  private state: DeviceState = {
    shizukuStatus: 'AUTHORIZED',
    tunnelState: 'CONNECTED',
    appStatus: 'FOREGROUND',
    vitals: {
      batteryPercent: 88,
      screenOn: true,
      networkType: 'WIFI',
      latencyMs: 14,
    },
    pairing: {
      scheme: 'wss',
      host: 'desktop.lan',
      port: 443,
      path: '/tunnel',
      token: 'mock-auth-token-12345',
      app: 'texure',
    },
    paired: true,
  };

  private logs: LogEntry[] = [
    {
      id: 'log-1',
      timestamp: new Date(Date.now() - 30000).toISOString(),
      level: 'INFO',
      message: 'Reverse WebSocket tunnel connected to desktop wss://desktop.lan:443/tunnel',
      source: 'tunnel',
    },
    {
      id: 'log-2',
      timestamp: new Date(Date.now() - 25000).toISOString(),
      level: 'INFO',
      message: 'Shizuku service ping OK: IPC version 13, uid 2000',
      source: 'shizuku',
    },
    {
      id: 'log-3',
      timestamp: new Date(Date.now() - 15000).toISOString(),
      level: 'DEBUG',
      message: 'Target app com.example.target is FOREGROUND (activity: .MainActivity)',
      source: 'lifecycle',
    },
  ];

  private listeners: Map<string, Set<Listener<any>>> = new Map();
  private tickerId: any = null;

  constructor() {
    // Lazy ticker: started on first interaction when not in Tauri environment
  }

  public startMockTicker() {
    if (typeof window === 'undefined' || this.tickerId) return;
    if (typeof window.__TAURI_INTERNALS__ !== 'undefined') return;
    this.tickerId = setInterval(() => {
      // Simulate slight latency fluctuation
      if (this.state.tunnelState === 'CONNECTED') {
        const jitter = Math.floor(Math.random() * 5) - 2;
        this.state.vitals.latencyMs = Math.max(5, this.state.vitals.latencyMs + jitter);
        this.emit('device_state_changed', { ...this.state });
      }
    }, 5000);
  }

  public stopMockTicker() {
    if (this.tickerId) {
      clearInterval(this.tickerId);
      this.tickerId = null;
    }
  }

  public emit<T>(event: string, payload: T) {
    const handlers = this.listeners.get(event);
    if (handlers) {
      handlers.forEach((h) => {
        try {
          h(payload);
        } catch (e) {
          console.error('Error in mock event listener:', e);
        }
      });
    }
  }

  public listen<T>(event: string, handler: Listener<T>): () => void {
    this.startMockTicker();
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(handler);
    return () => {
      this.listeners.get(event)?.delete(handler);
    };
  }

  public addLog(level: LogEntry['level'], message: string, source = 'system') {
    const entry: LogEntry = {
      id: `log-${Date.now()}-${Math.random().toString(36).substring(2, 6)}`,
      timestamp: new Date().toISOString(),
      level,
      message,
      source,
    };
    this.logs.push(entry);
    if (this.logs.length > 500) {
      this.logs.shift();
    }
    this.emit('tauri://event/log', entry);
    this.emit('log', entry);
  }

  public async invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    switch (cmd) {
      case 'get_device_state': {
        return { ...this.state } as unknown as T;
      }
      case 'pair_device': {
        const config = (args?.config || args?.payload) as PairingConfig;
        this.state.pairing = config;
        this.state.paired = true;
        this.state.tunnelState = 'CONNECTED';
        this.addLog('INFO', `Device paired with ${config.host}:${config.port}`, 'pairing');
        this.emit('device_state_changed', { ...this.state });
        return { ok: true } as unknown as T;
      }
      case 'wake_screen': {
        this.state.vitals.screenOn = true;
        this.addLog('INFO', 'Screen woken via Shizuku input keyevent 26', 'shizuku');
        this.emit('device_state_changed', { ...this.state });
        return { ok: true, state: 'screen_on' } as unknown as T;
      }
      case 'dismiss_keyguard': {
        this.addLog('INFO', 'Keyguard dismissed via Shizuku wm dismiss-keyguard', 'shizuku');
        this.emit('device_state_changed', { ...this.state });
        return { ok: true, state: 'unlocked' } as unknown as T;
      }
      case 'cold_start_app': {
        const pkg = (args?.package as string) || 'com.example.target';
        this.state.appStatus = 'FOREGROUND';
        this.addLog('INFO', `am start -n ${pkg}/.MainActivity launched`, 'lifecycle');
        this.emit('device_state_changed', { ...this.state });
        return { ok: true, bridge: 'online' } as unknown as T;
      }
      case 'force_stop_app': {
        const pkg = (args?.package as string) || 'com.example.target';
        this.state.appStatus = 'STOPPED';
        this.addLog('WARN', `am force-stop ${pkg} executed`, 'lifecycle');
        this.emit('device_state_changed', { ...this.state });
        return { ok: true } as unknown as T;
      }
      case 'reconnect_tunnel': {
        this.state.tunnelState = 'CONNECTING';
        this.addLog('INFO', 'Tunnel reconnecting to endpoint...', 'tunnel');
        this.emit('device_state_changed', { ...this.state });

        setTimeout(() => {
          this.state.tunnelState = 'CONNECTED';
          this.addLog('INFO', 'Tunnel reconnected successfully', 'tunnel');
          this.emit('device_state_changed', { ...this.state });
        }, 600);

        return { ok: true } as unknown as T;
      }
      case 'get_logs': {
        return [...this.logs] as unknown as T;
      }
      case 'clear_logs': {
        this.logs = [];
        this.emit('logs_cleared', {});
        return { ok: true } as unknown as T;
      }
      case 'export_logs': {
        const text = this.logs
          .map((l) => `[${l.timestamp}] [${l.level}] [${l.source || 'sys'}] ${l.message}`)
          .join('\n');
        return text as unknown as T;
      }
      case 'set_mock_state': {
        const patch = args?.patch as Partial<DeviceState>;
        if (patch) {
          const newVitals = patch.vitals ? { ...this.state.vitals, ...patch.vitals } : this.state.vitals;
          this.state = { ...this.state, ...patch, vitals: newVitals };
          this.emit('device_state_changed', { ...this.state });
        }
        return { ...this.state } as unknown as T;
      }
      default:
        console.warn(`[MockBackend] Unhandled command: ${cmd}`, args);
        throw new Error(`[MockBackend] Unknown or unhandled command: "${cmd}"`);
    }
  }

  // Helper for mock toolbar
  public updateState(updater: (prev: DeviceState) => DeviceState) {
    this.state = updater(this.state);
    this.emit('device_state_changed', { ...this.state });
  }

  public reset() {
    this.state = {
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
    this.logs = [];
    this.emit('device_state_changed', { ...this.state });
  }

  public getState(): DeviceState {
    return { ...this.state };
  }
}

export const mockBackend = new MockBackend();
