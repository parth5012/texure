import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { StatusDashboard } from '../components/StatusDashboard';
import { CompanionProvider } from '../context/CompanionContext';
import { mockBackend } from '../lib/mock-backend';

describe('StatusDashboard Component', () => {
  it('renders all telemetry cards and badges', () => {
    render(
      <CompanionProvider>
        <StatusDashboard />
      </CompanionProvider>
    );

    expect(screen.getByText('Companion Status Dashboard')).toBeInTheDocument();
    expect(screen.getByText('Shizuku Gateway')).toBeInTheDocument();
    expect(screen.getByText('Reverse WS Tunnel')).toBeInTheDocument();
    expect(screen.getByText('Target Application')).toBeInTheDocument();
    expect(screen.getByText('Device Vitals')).toBeInTheDocument();
    expect(screen.getByTestId('shizuku-badge-authorized')).toBeInTheDocument();
    expect(screen.getByTestId('tunnel-badge-connected')).toBeInTheDocument();
    expect(screen.getByTestId('app-badge-foreground')).toBeInTheDocument();
    expect(screen.getByTestId('screen-state-indicator')).toHaveTextContent('ON');
  });

  it('updates badges when mock backend state changes', async () => {
    render(
      <CompanionProvider>
        <StatusDashboard />
      </CompanionProvider>
    );

    // Wait for initial mount
    await waitFor(() => {
      expect(screen.getByTestId('shizuku-badge-authorized')).toBeInTheDocument();
    });

    await mockBackend.invoke('set_mock_state', {
      patch: {
        shizukuStatus: 'UNAUTHORIZED',
        tunnelState: 'ERROR',
        appStatus: 'STOPPED',
        vitals: {
          screenOn: false,
          batteryPercent: 15,
          networkType: 'CELLULAR',
          latencyMs: 120,
        },
      },
    });

    await waitFor(() => {
      expect(screen.getByTestId('shizuku-badge-unauthorized')).toBeInTheDocument();
      expect(screen.getByTestId('tunnel-badge-error')).toBeInTheDocument();
      expect(screen.getByTestId('app-badge-stopped')).toBeInTheDocument();
      expect(screen.getByTestId('screen-state-indicator')).toHaveTextContent('OFF');
      expect(screen.getByText('15%')).toBeInTheDocument();
      expect(screen.getByText('CELLULAR')).toBeInTheDocument();
    });
  });

  it('triggers refresh and reconnect actions', async () => {
    render(
      <CompanionProvider>
        <StatusDashboard />
      </CompanionProvider>
    );

    const refreshBtn = screen.getByTestId('dashboard-refresh-btn');
    fireEvent.click(refreshBtn);

    const reconnectBtn = screen.getByText('Reconnect');
    fireEvent.click(reconnectBtn);

    await waitFor(() => {
      expect(screen.getByTestId('tunnel-badge-connecting')).toBeInTheDocument();
    });
  });
});
