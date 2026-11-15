import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import App from '../App';

describe('Companion App Shell', () => {
  it('renders application header and mock toolbar', async () => {
    render(<App />);

    expect(screen.getByText('Texure Companion')).toBeInTheDocument();
    expect(screen.getByText(/Browser Mock Mode Active/i)).toBeInTheDocument();
    expect(screen.getByText('Device Status')).toBeInTheDocument();
  });

  it('switches between dashboard tabs', async () => {
    render(<App />);

    expect(screen.getByTestId('dashboard-view')).toBeInTheDocument();

    const controlsTab = screen.getByText('Manual Controls');
    fireEvent.click(controlsTab);

    await waitFor(() => {
      expect(screen.getByTestId('controls-view')).toBeInTheDocument();
    });

    const scannerTab = screen.getByText('QR Pairing');
    fireEvent.click(scannerTab);

    await waitFor(() => {
      expect(screen.getByTestId('scanner-view')).toBeInTheDocument();
    });

    const logsTab = screen.getByText('Live Logs');
    fireEvent.click(logsTab);

    await waitFor(() => {
      expect(screen.getByTestId('logs-view')).toBeInTheDocument();
    });
  });

  it('toggles mock simulator controls', async () => {
    render(<App />);

    const toggleBtn = screen.getByText(/Toggle Simulator Controls/i);
    fireEvent.click(toggleBtn);

    expect(screen.getByText('Shizuku Status')).toBeInTheDocument();
    expect(screen.getByText('Tunnel State')).toBeInTheDocument();
  });
});
