import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CompanionProvider } from '../context/CompanionContext';
import { LogViewer } from '../components/LogViewer';
import { mockBackend } from '../lib/mock-backend';

describe('LogViewer Component', () => {
  beforeEach(() => {
    mockBackend.reset();
    mockBackend.addLog('INFO', 'Test system started', 'sys');
    mockBackend.addLog('WARN', 'High latency warning detected', 'network');
    mockBackend.addLog('ERROR', 'Bridge connection timed out', 'tunnel');
  });

  const renderLogViewer = () => {
    return render(
      <CompanionProvider>
        <LogViewer />
      </CompanionProvider>
    );
  };

  it('renders log viewer UI components and initial log entries', async () => {
    renderLogViewer();

    await waitFor(() => {
      expect(screen.getByTestId('log-viewer')).toBeInTheDocument();
    });

    expect(screen.getByTestId('input-log-search')).toBeInTheDocument();
    expect(screen.getByTestId('select-log-level')).toBeInTheDocument();
    expect(screen.getByTestId('btn-autoscroll-toggle')).toBeInTheDocument();
    expect(screen.getByTestId('btn-copy-logs')).toBeInTheDocument();
    expect(screen.getByTestId('btn-export-logs')).toBeInTheDocument();
    expect(screen.getByTestId('btn-clear-logs')).toBeInTheDocument();

    await waitFor(() => {
      const rows = screen.getAllByTestId('log-entry-row');
      expect(rows.length).toBeGreaterThanOrEqual(3);
    });

    expect(screen.getByText('Test system started')).toBeInTheDocument();
    expect(screen.getByText('High latency warning detected')).toBeInTheDocument();
    expect(screen.getByText('Bridge connection timed out')).toBeInTheDocument();
  });

  it('filters log stream by severity level', async () => {
    renderLogViewer();

    await waitFor(() => {
      expect(screen.getByText('Test system started')).toBeInTheDocument();
    });

    const levelSelect = screen.getByTestId('select-log-level');

    // Filter to ERROR only
    fireEvent.change(levelSelect, { target: { value: 'ERROR' } });

    await waitFor(() => {
      expect(screen.getByText('Bridge connection timed out')).toBeInTheDocument();
      expect(screen.queryByText('Test system started')).not.toBeInTheDocument();
      expect(screen.queryByText('High latency warning detected')).not.toBeInTheDocument();
    });

    // Filter to WARN only
    fireEvent.change(levelSelect, { target: { value: 'WARN' } });

    await waitFor(() => {
      expect(screen.getByText('High latency warning detected')).toBeInTheDocument();
      expect(screen.queryByText('Bridge connection timed out')).not.toBeInTheDocument();
    });

    // Filter back to ALL
    fireEvent.change(levelSelect, { target: { value: 'ALL' } });

    await waitFor(() => {
      expect(screen.getByText('Test system started')).toBeInTheDocument();
      expect(screen.getByText('High latency warning detected')).toBeInTheDocument();
      expect(screen.getByText('Bridge connection timed out')).toBeInTheDocument();
    });
  });

  it('filters log stream by search query', async () => {
    renderLogViewer();

    await waitFor(() => {
      expect(screen.getByText('Test system started')).toBeInTheDocument();
    });

    const searchInput = screen.getByTestId('input-log-search');
    fireEvent.change(searchInput, { target: { value: 'latency' } });

    await waitFor(() => {
      expect(screen.getByText('High latency warning detected')).toBeInTheDocument();
      expect(screen.queryByText('Test system started')).not.toBeInTheDocument();
      expect(screen.queryByText('Bridge connection timed out')).not.toBeInTheDocument();
    });

    // Search query with no match shows empty state
    fireEvent.change(searchInput, { target: { value: 'nonexistent-string-xyz' } });

    await waitFor(() => {
      expect(screen.getByTestId('log-empty-state')).toBeInTheDocument();
      expect(screen.getByText(/no event log entries found/i)).toBeInTheDocument();
    });
  });

  it('toggles auto-scroll mode', async () => {
    renderLogViewer();

    await waitFor(() => {
      expect(screen.getByTestId('btn-autoscroll-toggle')).toBeInTheDocument();
    });

    const toggleBtn = screen.getByTestId('btn-autoscroll-toggle');
    expect(toggleBtn).toHaveTextContent(/auto-scroll on/i);

    fireEvent.click(toggleBtn);
    expect(toggleBtn).toHaveTextContent(/paused/i);

    fireEvent.click(toggleBtn);
    expect(toggleBtn).toHaveTextContent(/auto-scroll on/i);
  });

  it('clears logs when clear button is clicked', async () => {
    renderLogViewer();

    await waitFor(() => {
      expect(screen.getByText('Test system started')).toBeInTheDocument();
    });

    const clearBtn = screen.getByTestId('btn-clear-logs');
    fireEvent.click(clearBtn);

    await waitFor(() => {
      expect(screen.getByTestId('log-empty-state')).toBeInTheDocument();
      expect(screen.queryByText('Test system started')).not.toBeInTheDocument();
    });
  });

  it('handles copy and export actions', async () => {
    // Mock navigator.clipboard
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextMock,
      },
    });

    renderLogViewer();

    await waitFor(() => {
      expect(screen.getByTestId('btn-copy-logs')).toBeInTheDocument();
    });

    const copyBtn = screen.getByTestId('btn-copy-logs');
    fireEvent.click(copyBtn);

    await waitFor(() => {
      expect(writeTextMock).toHaveBeenCalled();
    });

    // Export logs
    const exportBtn = screen.getByTestId('btn-export-logs');
    expect(exportBtn).not.toBeDisabled();
    fireEvent.click(exportBtn);
  });
});
