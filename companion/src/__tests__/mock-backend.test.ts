import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mockBackend } from '../lib/mock-backend';
import { DeviceState } from '../types';

describe('MockBackend', () => {
  beforeEach(() => {
    // Reset state to known baseline
    mockBackend.invoke('set_mock_state', {
      patch: {
        shizukuStatus: 'AUTHORIZED',
        tunnelState: 'CONNECTED',
        appStatus: 'FOREGROUND',
        vitals: {
          batteryPercent: 88,
          screenOn: true,
          networkType: 'WIFI',
          latencyMs: 14,
        },
      },
    });
  });

  afterEach(() => {
    mockBackend.stopMockTicker();
  });

  it('retrieves device state cleanly', async () => {
    const state = await mockBackend.invoke<DeviceState>('get_device_state');
    expect(state).toBeDefined();
    expect(state.shizukuStatus).toBe('AUTHORIZED');
    expect(state.tunnelState).toBe('CONNECTED');
    expect(state.vitals.screenOn).toBe(true);
  });

  it('updates screen state on wake_screen', async () => {
    await mockBackend.invoke('set_mock_state', { patch: { vitals: { screenOn: false } } });
    let state = await mockBackend.invoke<DeviceState>('get_device_state');
    expect(state.vitals.screenOn).toBe(false);

    await mockBackend.invoke('wake_screen');
    state = await mockBackend.invoke<DeviceState>('get_device_state');
    expect(state.vitals.screenOn).toBe(true);
  });

  it('toggles tunnel reconnect state', async () => {
    const res = await mockBackend.invoke<{ ok: boolean }>('reconnect_tunnel');
    expect(res.ok).toBe(true);
    const state = await mockBackend.invoke<DeviceState>('get_device_state');
    expect(['CONNECTING', 'CONNECTED']).toContain(state.tunnelState);
  });

  it('manages logs stream and clears', async () => {
    mockBackend.addLog('INFO', 'Test log message', 'test');
    const logs = await mockBackend.invoke<any[]>('get_logs');
    expect(logs.some((l) => l.message === 'Test log message')).toBe(true);

    await mockBackend.invoke('clear_logs');
    const emptyLogs = await mockBackend.invoke<any[]>('get_logs');
    expect(emptyLogs.length).toBe(0);
  });
});
