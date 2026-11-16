import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CompanionProvider } from '../context/CompanionContext';
import { ControlPanel } from '../components/ControlPanel';
import { mockBackend } from '../lib/mock-backend';

describe('ControlPanel Component', () => {
  beforeEach(() => {
    mockBackend.reset();
  });

  const renderControlPanel = () => {
    return render(
      <CompanionProvider>
        <ControlPanel />
      </CompanionProvider>
    );
  };

  it('renders all control action buttons and package input', async () => {
    renderControlPanel();

    await waitFor(() => {
      expect(screen.getByTestId('control-panel')).toBeInTheDocument();
    });

    expect(screen.getByTestId('btn-wake-screen')).toBeInTheDocument();
    expect(screen.getByTestId('btn-dismiss-keyguard')).toBeInTheDocument();
    expect(screen.getByTestId('btn-cold-start')).toBeInTheDocument();
    expect(screen.getByTestId('btn-force-stop')).toBeInTheDocument();
    expect(screen.getByTestId('btn-reconnect-tunnel')).toBeInTheDocument();
    expect(screen.getByTestId('input-package-name')).toHaveValue('com.example.target');
  });

  it('dispatches wake screen and displays success toast', async () => {
    renderControlPanel();

    await waitFor(() => {
      expect(screen.getByTestId('btn-wake-screen')).toBeInTheDocument();
    });

    const wakeBtn = screen.getByTestId('btn-wake-screen');
    fireEvent.click(wakeBtn);

    await waitFor(() => {
      expect(screen.getByTestId('toast-notification')).toBeInTheDocument();
      expect(screen.getByTestId('toast-message')).toHaveTextContent(/keyevent 26/i);
    });
  });

  it('dispatches dismiss keyguard action', async () => {
    renderControlPanel();

    await waitFor(() => {
      expect(screen.getByTestId('btn-dismiss-keyguard')).toBeInTheDocument();
    });

    const dismissBtn = screen.getByTestId('btn-dismiss-keyguard');
    fireEvent.click(dismissBtn);

    await waitFor(() => {
      expect(screen.getByTestId('toast-notification')).toBeInTheDocument();
      expect(screen.getByTestId('toast-message')).toHaveTextContent(/dismiss-keyguard/i);
    });
  });

  it('updates package name and dispatches cold start and force stop actions', async () => {
    renderControlPanel();

    await waitFor(() => {
      expect(screen.getByTestId('input-package-name')).toBeInTheDocument();
    });

    const input = screen.getByTestId('input-package-name');
    fireEvent.change(input, { target: { value: 'com.texure.customapp' } });
    expect(input).toHaveValue('com.texure.customapp');

    const coldStartBtn = screen.getByTestId('btn-cold-start');
    fireEvent.click(coldStartBtn);

    await waitFor(() => {
      expect(screen.getByTestId('toast-notification')).toBeInTheDocument();
      expect(screen.getByTestId('toast-message')).toHaveTextContent(/com.texure.customapp/i);
    });

    // Dismiss first toast
    const dismissBtn = screen.getByTestId('toast-dismiss');
    fireEvent.click(dismissBtn);

    // Force stop
    const forceStopBtn = screen.getByTestId('btn-force-stop');
    fireEvent.click(forceStopBtn);

    await waitFor(() => {
      expect(screen.getByTestId('toast-notification')).toBeInTheDocument();
      expect(screen.getByTestId('toast-message')).toHaveTextContent(/killed/i);
    });
  });

  it('dispatches reconnect tunnel action', async () => {
    renderControlPanel();

    await waitFor(() => {
      expect(screen.getByTestId('btn-reconnect-tunnel')).toBeInTheDocument();
    });

    const reconnectBtn = screen.getByTestId('btn-reconnect-tunnel');
    fireEvent.click(reconnectBtn);

    await waitFor(() => {
      expect(screen.getByTestId('toast-notification')).toBeInTheDocument();
      expect(screen.getByTestId('toast-message')).toHaveTextContent(/reconnect/i);
    });
  });

  it('displays error toast when action fails', async () => {
    const originalInvoke = mockBackend.invoke.bind(mockBackend);
    vi.spyOn(mockBackend, 'invoke').mockImplementation(async (cmd, args) => {
      if (cmd === 'wake_screen') {
        throw new Error('Shizuku IPC failure');
      }
      return originalInvoke(cmd, args);
    });

    renderControlPanel();

    await waitFor(() => {
      expect(screen.getByTestId('btn-wake-screen')).toBeInTheDocument();
    });

    const wakeBtn = screen.getByTestId('btn-wake-screen');
    fireEvent.click(wakeBtn);

    await waitFor(() => {
      expect(screen.getByTestId('toast-notification')).toBeInTheDocument();
      expect(screen.getByTestId('toast-message')).toHaveTextContent(/Shizuku IPC failure/i);
    });
  });

  it('validates package name and disables lifecycle buttons for invalid syntax', async () => {
    renderControlPanel();

    const input = screen.getByTestId('input-package-name');
    const coldStartBtn = screen.getByTestId('btn-cold-start');
    const forceStopBtn = screen.getByTestId('btn-force-stop');

    // Initially valid
    expect(coldStartBtn).not.toBeDisabled();
    expect(forceStopBtn).not.toBeDisabled();

    // Type invalid package name
    fireEvent.change(input, { target: { value: 'invalid-package-name!' } });

    await waitFor(() => {
      expect(screen.getByTestId('package-validation-error')).toBeInTheDocument();
      expect(coldStartBtn).toBeDisabled();
      expect(forceStopBtn).toBeDisabled();
    });

    // Fix package name
    fireEvent.change(input, { target: { value: 'com.valid.app_one' } });

    await waitFor(() => {
      expect(screen.queryByTestId('package-validation-error')).not.toBeInTheDocument();
      expect(coldStartBtn).not.toBeDisabled();
      expect(forceStopBtn).not.toBeDisabled();
    });
  });
});
