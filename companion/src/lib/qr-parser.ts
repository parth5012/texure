import { PairingConfig } from '../types';

export interface ParseResult {
  valid: boolean;
  config?: PairingConfig;
  error?: string;
}

/**
 * Validates and parses QR payload into standard PairingConfig.
 * Strictly adheres to BB-R11 contract (contract/fixtures/pairing_qr_payload.json).
 */
export function parsePairingPayload(raw: string): ParseResult {
  const trimmed = raw.trim();
  if (!trimmed) {
    return { valid: false, error: 'Empty payload' };
  }

  try {
    // Attempt parsing as JSON
    const data = JSON.parse(trimmed);

    // Format 1: Direct JSON schema per contract/fixtures/pairing_qr_payload.json
    // { scheme: "wss", host: "...", port: 443, path: "/tunnel", token: "...", app: "texure" }
    if (data.scheme || data.host) {
      if (data.scheme !== 'wss') {
        return { valid: false, error: 'Plain or insecure ws endpoint is rejected. wss:// required.' };
      }
      if (!data.token || typeof data.token !== 'string' || data.token.trim() === '') {
        return { valid: false, error: 'Missing pairing token' };
      }
      if (!data.host || typeof data.host !== 'string') {
        return { valid: false, error: 'Missing host' };
      }

      const port = Number(data.port) || 443;
      const path = data.path ? (data.path.startsWith('/') ? data.path : `/${data.path}`) : '/tunnel';
      const url = `wss://${data.host}:${port}${path}`;

      return {
        valid: true,
        config: {
          scheme: 'wss',
          host: data.host,
          port,
          path,
          token: data.token,
          app: data.app || 'texure',
          url,
        },
      };
    }

    // Format 2: JSON with URL and token
    // { url: "wss://desktop.example:443/tunnel", token: "..." }
    if (data.url) {
      const parsedUrl = new URL(data.url);
      if (parsedUrl.protocol !== 'wss:') {
        return { valid: false, error: 'Endpoint must use wss:// scheme' };
      }
      const token = data.token || parsedUrl.searchParams.get('token');
      if (!token || typeof token !== 'string' || token.trim() === '') {
        return { valid: false, error: 'Missing pairing token' };
      }

      const port = parsedUrl.port ? Number(parsedUrl.port) : 443;
      return {
        valid: true,
        config: {
          scheme: 'wss',
          host: parsedUrl.hostname,
          port,
          path: parsedUrl.pathname || '/tunnel',
          token,
          app: data.app || 'texure',
          url: data.url,
        },
      };
    }

    return { valid: false, error: 'JSON payload missing required endpoint details' };
  } catch {
    // Fallback: raw URL string: "wss://host:port/path?token=..."
    try {
      const parsedUrl = new URL(trimmed);
      if (parsedUrl.protocol !== 'wss:') {
        return { valid: false, error: 'Endpoint must use wss:// scheme' };
      }
      const token = parsedUrl.searchParams.get('token');
      if (!token) {
        return { valid: false, error: 'Missing pairing token parameter' };
      }

      const port = parsedUrl.port ? Number(parsedUrl.port) : 443;
      return {
        valid: true,
        config: {
          scheme: 'wss',
          host: parsedUrl.hostname,
          port,
          path: parsedUrl.pathname || '/tunnel',
          token,
          app: 'texure',
          url: trimmed,
        },
      };
    } catch {
      return { valid: false, error: 'Malformed QR pairing payload format' };
    }
  }
}
