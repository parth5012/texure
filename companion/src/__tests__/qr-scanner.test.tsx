import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QrScanner } from '../components/QrScanner';
import { parsePairingPayload } from '../lib/qr-parser';
import { CompanionProvider } from '../context/CompanionContext';

describe('QR Parser (BB-R11 golden pairing spec)', () => {
  it('parses valid structured JSON QR payload', () => {
    const payload = JSON.stringify({
      scheme: 'wss',
      host: 'desktop.example',
      port: 443,
      path: '/tunnel',
      token: 'golden-token-bb-r11',
      app: 'texure',
    });

    const res = parsePairingPayload(payload);
    expect(res.valid).toBe(true);
    expect(res.config?.host).toBe('desktop.example');
    expect(res.config?.port).toBe(443);
    expect(res.config?.scheme).toBe('wss');
    expect(res.config?.token).toBe('golden-token-bb-r11');
    expect(res.config?.url).toBe('wss://desktop.example:443/tunnel');
  });

  it('rejects plain ws endpoint per contract BB-R11', () => {
    const payload = JSON.stringify({
      scheme: 'ws',
      host: 'desktop.example',
      port: 8080,
      path: '/tunnel',
      token: 'some-token',
      app: 'texure',
    });

    const res = parsePairingPayload(payload);
    expect(res.valid).toBe(false);
    expect(res.error).toMatch(/wss:\/\/ required/i);
  });

  it('rejects payload missing token per contract BB-R11', () => {
    const payload = JSON.stringify({
      scheme: 'wss',
      host: 'desktop.example',
      port: 443,
      path: '/tunnel',
      app: 'texure',
    });

    const res = parsePairingPayload(payload);
    expect(res.valid).toBe(false);
    expect(res.error).toMatch(/missing pairing token/i);
  });

  it('parses JSON with explicit URL format', () => {
    const payload = JSON.stringify({
      url: 'wss://desktop.lan:8443/custom-tunnel',
      token: 'auth-secret-xyz',
    });

    const res = parsePairingPayload(payload);
    expect(res.valid).toBe(true);
    expect(res.config?.host).toBe('desktop.lan');
    expect(res.config?.port).toBe(8443);
    expect(res.config?.path).toBe('/custom-tunnel');
    expect(res.config?.token).toBe('auth-secret-xyz');
  });
});

describe('QrScanner Component', () => {
  it('renders scanner UI elements and camera scan button', () => {
    render(
      <CompanionProvider>
        <QrScanner />
      </CompanionProvider>
    );

    expect(screen.getByText('Pair Remote Gateway')).toBeInTheDocument();
    expect(screen.getByText(/Start Camera Scan/i)).toBeInTheDocument();
    expect(screen.getByText(/Drag and drop a QR code image here/i)).toBeInTheDocument();
  });

  it('handles golden pairing payload via mock test button', async () => {
    const onPairSuccess = vi.fn();
    render(
      <CompanionProvider>
        <QrScanner onPairSuccess={onPairSuccess} />
      </CompanionProvider>
    );

    const btn = screen.getByTestId('load-fixture-qr-btn');
    fireEvent.click(btn);

    await waitFor(() => {
      expect(screen.getByTestId('pair-success-state')).toBeInTheDocument();
      expect(screen.getByText(/Pairing Successful!/i)).toBeInTheDocument();
      expect(screen.getByText(/wss:\/\/desktop\.example:443\/tunnel/i)).toBeInTheDocument();
    });

    expect(onPairSuccess).toHaveBeenCalledWith(
      expect.objectContaining({
        scheme: 'wss',
        host: 'desktop.example',
        token: 'mock-bearer-token-bb-r11',
      })
    );
  });

  it('processes uploaded JSON pairing file with invalid payload and shows error', async () => {
    render(
      <CompanionProvider>
        <QrScanner />
      </CompanionProvider>
    );

    const file = new File(
      [JSON.stringify({ scheme: 'ws', host: 'insecure.host', token: 'abc' })],
      'pairing.json',
      { type: 'application/json' }
    );

    const input = screen.getByTestId('file-upload-input');
    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(() => {
      expect(screen.getByTestId('parse-error-message')).toBeInTheDocument();
      expect(screen.getByText(/wss:\/\/ required/i)).toBeInTheDocument();
    });
  });
});
