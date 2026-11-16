import { PairingConfig } from '../types';

export const DEFAULT_PORT = 443;
export const DEFAULT_PATH = '/tunnel';
export const DEFAULT_APP = 'texure';

export interface ParseResult {
  valid: boolean;
  config?: PairingConfig;
  error?: string;
}

function normalizePath(rawPath?: string): string {
  if (!rawPath || rawPath.trim() === '') {
    return DEFAULT_PATH;
  }
  return rawPath.startsWith('/') ? rawPath : `/${rawPath}`;
}

function isValidPort(port: number): boolean {
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

function cleanHost(rawHost: string): { host: string; port?: number } {
  const trimmed = rawHost.trim();
  if (trimmed.startsWith('[')) {
    const endBracket = trimmed.indexOf(']');
    if (endBracket !== -1) {
      const host = trimmed.substring(0, endBracket + 1);
      const after = trimmed.substring(endBracket + 1);
      if (after.startsWith(':')) {
        const port = Number(after.substring(1));
        return { host, port: isValidPort(port) ? port : undefined };
      }
      return { host };
    }
  }
  if (trimmed.includes(':')) {
    const parts = trimmed.split(':');
    const port = Number(parts[1]);
    return { host: parts[0], port: isValidPort(port) ? port : undefined };
  }
  return { host: trimmed };
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

      const { host, port: embeddedPort } = cleanHost(data.host);
      let port = Number(data.port) || embeddedPort || DEFAULT_PORT;
      if (!isValidPort(port)) {
        return { valid: false, error: `Invalid port number: ${data.port}` };
      }

      const path = normalizePath(data.path);
      const app = data.app && typeof data.app === 'string' ? data.app : DEFAULT_APP;
      const url = `wss://${host}:${port}${path}`;

      return {
        valid: true,
        config: {
          scheme: 'wss',
          host,
          port,
          path,
          token: data.token,
          app,
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

      const port = parsedUrl.port ? Number(parsedUrl.port) : DEFAULT_PORT;
      if (!isValidPort(port)) {
        return { valid: false, error: `Invalid URL port: ${parsedUrl.port}` };
      }

      const app = data.app || parsedUrl.searchParams.get('app') || DEFAULT_APP;
      const path = normalizePath(parsedUrl.pathname);

      return {
        valid: true,
        config: {
          scheme: 'wss',
          host: parsedUrl.hostname,
          port,
          path,
          token,
          app,
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

      const port = parsedUrl.port ? Number(parsedUrl.port) : DEFAULT_PORT;
      if (!isValidPort(port)) {
        return { valid: false, error: `Invalid port in URL: ${parsedUrl.port}` };
      }

      const app = parsedUrl.searchParams.get('app') || DEFAULT_APP;
      const path = normalizePath(parsedUrl.pathname);

      return {
        valid: true,
        config: {
          scheme: 'wss',
          host: parsedUrl.hostname,
          port,
          path,
          token,
          app,
          url: trimmed,
        },
      };
    } catch {
      return { valid: false, error: 'Malformed QR pairing payload format' };
    }
  }
}
