import React, { useState, useRef, useEffect, useCallback } from 'react';
import { Camera, Upload, AlertCircle, CheckCircle2, RefreshCw, X, ShieldCheck, HelpCircle, Radio } from 'lucide-react';
import { PairingConfig } from '../types';
import { parsePairingPayload, DEFAULT_APP, DEFAULT_PATH, DEFAULT_PORT } from '../lib/qr-parser';
import { useCompanion } from '../context/CompanionContext';

interface DetectedBarcode {
  rawValue: string;
  format?: string;
}

interface BarcodeDetectorInstance {
  detect(source: HTMLVideoElement | ImageBitmap): Promise<DetectedBarcode[]>;
}

declare global {
  interface Window {
    BarcodeDetector?: {
      new (options?: { formats: string[] }): BarcodeDetectorInstance;
    };
  }
}

export const BB_R11_TEST_PAYLOAD = JSON.stringify({
  scheme: 'wss',
  host: 'desktop.example',
  port: DEFAULT_PORT,
  path: DEFAULT_PATH,
  token: 'mock-bearer-token-bb-r11',
  app: DEFAULT_APP,
});

interface QrScannerProps {
  onPairSuccess?: (config: PairingConfig) => void;
  onCancel?: () => void;
}

async function readFileAsText(file: File): Promise<string> {
  if (typeof file.text === 'function') {
    return await file.text();
  }
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error || new Error('Failed to read file'));
    reader.readAsText(file);
  });
}

export const QrScanner: React.FC<QrScannerProps> = ({ onPairSuccess, onCancel }) => {
  const { pairDevice, isBrowserMock } = useCompanion();
  const videoRef = useRef<HTMLVideoElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [scanning, setScanning] = useState(false);
  const [cameraError, setCameraError] = useState<string | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [successConfig, setSuccessConfig] = useState<PairingConfig | null>(null);
  const [dragActive, setDragActive] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);

  const streamRef = useRef<MediaStream | null>(null);
  const animFrameRef = useRef<number | null>(null);
  const mountedRef = useRef(true);
  const detectorRef = useRef<any>(null);

  const stopCamera = useCallback(() => {
    if (animFrameRef.current) {
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }
    setScanning(false);
  }, []);

  const handlePayloadResult = useCallback(
    async (rawText: string) => {
      setIsProcessing(true);
      setParseError(null);
      const res = parsePairingPayload(rawText);

      if (!res.valid || !res.config) {
        setParseError(res.error || 'Invalid QR payload format');
        setIsProcessing(false);
        return;
      }

      const paired = await pairDevice(res.config);
      if (!paired) {
        setParseError('Failed to establish connection with device gateway');
        setIsProcessing(false);
        return;
      }

      setSuccessConfig(res.config);
      if (onPairSuccess) {
        onPairSuccess(res.config);
      }
      setIsProcessing(false);
      stopCamera();
    },
    [pairDevice, onPairSuccess, stopCamera]
  );

  const scanFrame = useCallback(async () => {
    if (!videoRef.current || !scanning || successConfig) return;

    const video = videoRef.current;
    if (video.readyState === video.HAVE_ENOUGH_DATA) {
      if (typeof window.BarcodeDetector !== 'undefined') {
        try {
          if (!detectorRef.current) {
            detectorRef.current = new window.BarcodeDetector({ formats: ['qr_code'] });
          }
          const barcodes = await detectorRef.current.detect(video);
          if (barcodes.length > 0 && barcodes[0].rawValue) {
            handlePayloadResult(barcodes[0].rawValue);
            return;
          }
        } catch (e) {
          console.debug('BarcodeDetector error, will retry or fallback', e);
        }
      }
    }

    if (scanning && !successConfig) {
      animFrameRef.current = requestAnimationFrame(scanFrame);
    }
  }, [scanning, successConfig, handlePayloadResult]);

  const startCamera = useCallback(async () => {
    setCameraError(null);
    setParseError(null);
    stopCamera();

    try {
      if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
        throw new Error('Camera access not supported in this environment. Please use file upload.');
      }

      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment', width: { ideal: 640 }, height: { ideal: 480 } },
      });

      if (!mountedRef.current) {
        stream.getTracks().forEach((track) => track.stop());
        return;
      }

      streamRef.current = stream;
      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        try {
          await videoRef.current.play();
        } catch (playErr) {
          console.warn('Video play error:', playErr);
        }
      }
      setScanning(true);
    } catch (err: any) {
      console.warn('Camera start error:', err);
      const isDenied = err.name === 'NotAllowedError' || err.name === 'PermissionDeniedError';
      setCameraError(
        isDenied
          ? 'Camera access was denied. Please check your browser permissions or use the file upload fallback below.'
          : err.message || 'Unable to access camera. Please use file upload fallback.'
      );
      setScanning(false);
    }
  }, [stopCamera]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      stopCamera();
    };
  }, [stopCamera]);

  useEffect(() => {
    if (scanning && !successConfig) {
      animFrameRef.current = requestAnimationFrame(scanFrame);
    }
    return () => {
      if (animFrameRef.current) {
        cancelAnimationFrame(animFrameRef.current);
        animFrameRef.current = null;
      }
    };
  }, [scanning, successConfig, scanFrame]);

  const processImageFile = async (file: File) => {
    setIsProcessing(true);
    setParseError(null);

    try {
      if (file.type === 'application/json' || file.name.endsWith('.json') || file.type.startsWith('text/')) {
        const text = await readFileAsText(file);
        await handlePayloadResult(text);
        return;
      }

      if (typeof window.BarcodeDetector !== 'undefined') {
        try {
          const imageBitmap = await createImageBitmap(file);
          const detector = new window.BarcodeDetector({ formats: ['qr_code'] });
          const barcodes = await detector.detect(imageBitmap);
          if (barcodes.length > 0 && barcodes[0].rawValue) {
            await handlePayloadResult(barcodes[0].rawValue);
            return;
          }
        } catch (bitmapErr) {
          console.debug('BarcodeDetector bitmap decode error:', bitmapErr);
        }
      }

      let imgUrl: string | null = null;
      try {
        const { BrowserQRCodeReader } = await import('@zxing/browser');
        const codeReader = new BrowserQRCodeReader();
        imgUrl = URL.createObjectURL(file);
        const result = await codeReader.decodeFromImageUrl(imgUrl);

        if (result && result.getText()) {
          await handlePayloadResult(result.getText());
          return;
        }
      } catch (zxingErr) {
        console.debug('ZXing decoding failed:', zxingErr);
      } finally {
        if (imgUrl) {
          URL.revokeObjectURL(imgUrl);
        }
      }

      setParseError('QR code is unreadable or damaged. Please try another image or upload a pairing JSON fixture.');
    } catch (err: any) {
      setParseError(`Failed to process file: ${err.message || err}`);
    } finally {
      setIsProcessing(false);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragActive(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      processImageFile(e.dataTransfer.files[0]);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900 dark:text-zinc-100 flex items-center space-x-2">
            <Camera className="w-5 h-5 text-indigo-500 dark:text-indigo-400" />
            <span>Pair Remote Gateway</span>
          </h2>
          <p className="text-xs text-slate-600 dark:text-zinc-400 mt-1">
            Authenticate and establish a secure reverse WebSocket connection between phone and desktop bridge.
          </p>
        </div>
        {onCancel && (
          <button
            onClick={onCancel}
            className="p-1.5 rounded-lg text-slate-400 hover:text-slate-700 dark:hover:text-zinc-200 transition"
            aria-label="Cancel"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Main 2-Column Responsive Deck */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-stretch">
        {/* Left Column: Viewfinder & Dropzone (7 Cols) */}
        <div className="lg:col-span-7 glass-panel rounded-2xl overflow-hidden p-6 shadow-xs flex flex-col justify-between">
          {successConfig ? (
            <div data-testid="pair-success-state" className="text-center py-10 space-y-4">
              <div className="w-16 h-16 rounded-2xl bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center mx-auto text-emerald-500 dark:text-emerald-400 shadow-md shadow-emerald-500/10">
                <CheckCircle2 className="w-8 h-8" />
              </div>
              <div>
                <h3 className="text-base font-semibold text-slate-900 dark:text-zinc-100">Pairing Successful!</h3>
                <p className="text-xs text-slate-600 dark:text-zinc-400 mt-1">
                  Connected to endpoint: <span className="font-mono text-indigo-600 dark:text-indigo-400 font-semibold">{successConfig.url}</span>
                </p>
              </div>
              <div className="inline-flex items-center space-x-2 text-xs bg-slate-100 dark:bg-zinc-800/80 px-3.5 py-1.5 rounded-xl border border-slate-200 dark:border-zinc-700">
                <ShieldCheck className="w-4 h-4 text-emerald-500" />
                <span className="text-slate-700 dark:text-zinc-300 font-mono text-xs font-medium">Token: {successConfig.token.substring(0, 16)}...</span>
              </div>
              <div>
                <button
                  onClick={() => {
                    setSuccessConfig(null);
                    startCamera();
                  }}
                  className="text-xs text-indigo-600 dark:text-indigo-400 hover:text-indigo-700 dark:hover:text-indigo-300 font-semibold underline"
                >
                  Scan Another QR
                </button>
              </div>
            </div>
          ) : (
            <div className="space-y-4">
              {/* Viewfinder or Camera Feed */}
              <div className={`relative aspect-video rounded-2xl overflow-hidden flex flex-col items-center justify-center shadow-md transition-colors ${
                scanning
                  ? 'bg-slate-900 border border-slate-800'
                  : 'bg-slate-100 dark:bg-zinc-950/80 border border-slate-200/90 dark:border-zinc-800'
              }`}>
                <video
                  ref={videoRef}
                  playsInline
                  muted
                  className={`w-full h-full object-cover ${scanning ? 'block' : 'hidden'}`}
                />
                {scanning ? (
                  <div className="absolute inset-0 pointer-events-none flex items-center justify-center">
                    <div className="w-48 h-48 border border-indigo-500/40 rounded-xl relative shadow-glow-indigo">
                      <div className="absolute top-0 left-0 w-4 h-4 border-t-2 border-l-2 border-indigo-400 rounded-tl-sm -mt-0.5 -ml-0.5" />
                      <div className="absolute top-0 right-0 w-4 h-4 border-t-2 border-r-2 border-indigo-400 rounded-tr-sm -mt-0.5 -mr-0.5" />
                      <div className="absolute bottom-0 left-0 w-4 h-4 border-b-2 border-l-2 border-indigo-400 rounded-bl-sm -mb-0.5 -ml-0.5" />
                      <div className="absolute bottom-0 right-0 w-4 h-4 border-b-2 border-r-2 border-indigo-400 rounded-br-sm -mb-0.5 -mr-0.5" />
                      <div className="w-full h-0.5 bg-gradient-to-r from-transparent via-cyan-400 to-transparent shadow-lg shadow-cyan-400 absolute animate-laser" />
                    </div>
                  </div>
                ) : (
                  <div className="text-center p-6 space-y-3 relative w-full h-full flex flex-col items-center justify-center">
                    {/* High-contrast technical optical reticle frame */}
                    <div className="absolute inset-4 pointer-events-none border border-slate-300/80 dark:border-zinc-700/80 rounded-xl">
                      <div className="absolute -top-0.5 -left-0.5 w-3.5 h-3.5 border-t-2 border-l-2 border-indigo-600 dark:border-indigo-400 rounded-tl-sm" />
                      <div className="absolute -top-0.5 -right-0.5 w-3.5 h-3.5 border-t-2 border-r-2 border-indigo-600 dark:border-indigo-400 rounded-tr-sm" />
                      <div className="absolute -bottom-0.5 -left-0.5 w-3.5 h-3.5 border-b-2 border-l-2 border-indigo-600 dark:border-indigo-400 rounded-bl-sm" />
                      <div className="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 border-b-2 border-r-2 border-indigo-600 dark:border-indigo-400 rounded-br-sm" />
                    </div>

                    <div className="w-12 h-12 rounded-2xl bg-white dark:bg-zinc-900 border border-slate-200 dark:border-zinc-800 flex items-center justify-center mx-auto text-slate-700 dark:text-zinc-300 shadow-xs relative z-10">
                      <Camera className="w-6 h-6" />
                    </div>
                    <div className="relative z-10">
                      <p className="text-xs text-slate-800 dark:text-zinc-200 font-medium">Camera preview is currently inactive.</p>
                      <p className="text-xs text-slate-500 dark:text-zinc-400 mt-0.5">Use camera to scan QR or upload fixture below</p>
                    </div>
                    <button
                      onClick={startCamera}
                      className="relative z-10 px-4 py-2 bg-gradient-to-r from-indigo-500 to-indigo-600 hover:from-indigo-400 hover:to-indigo-500 text-white rounded-xl text-xs font-medium transition inline-flex items-center space-x-2 shadow-sm shadow-indigo-500/25 active:scale-[0.99]"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      <span>Start Camera Scan</span>
                    </button>
                  </div>
                )}

                {scanning && (
                  <button
                    onClick={stopCamera}
                    className="absolute top-3 right-3 px-2.5 py-1 bg-zinc-900/90 hover:bg-zinc-800 rounded-lg text-zinc-200 text-xs border border-zinc-700/80 shadow-md backdrop-blur-md"
                  >
                    Stop Camera
                  </button>
                )}
              </div>

              {cameraError && (
                <div className="p-3.5 bg-amber-500/10 border border-amber-500/30 rounded-xl text-xs text-amber-700 dark:text-amber-300 flex items-start space-x-2.5">
                  <AlertCircle className="w-4 h-4 shrink-0 text-amber-500 mt-0.5" />
                  <span>{cameraError}</span>
                </div>
              )}

              {parseError && (
                <div data-testid="parse-error-message" className="p-3.5 bg-rose-500/10 border border-rose-500/30 rounded-xl text-xs text-rose-700 dark:text-rose-300 flex items-start space-x-2.5">
                  <AlertCircle className="w-4 h-4 shrink-0 text-rose-500 mt-0.5" />
                  <span>{parseError}</span>
                </div>
              )}

              {/* Drag & drop / File Upload Fallback */}
              <div
                role="button"
                tabIndex={0}
                aria-label="Upload QR code fixture image or JSON"
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    fileInputRef.current?.click();
                  }
                }}
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragActive(true);
                }}
                onDragLeave={() => setDragActive(false)}
                onDrop={handleDrop}
                onClick={() => fileInputRef.current?.click()}
                className={`border-2 border-dashed rounded-xl p-5 text-center cursor-pointer transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 ${
                  dragActive
                    ? 'border-indigo-500 bg-indigo-50/50 dark:bg-indigo-950/30'
                    : 'border-slate-300 dark:border-zinc-700 hover:border-indigo-500 dark:hover:border-zinc-500 bg-slate-50/60 dark:bg-zinc-950/40'
                }`}
              >
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="image/*,application/json"
                  className="hidden"
                  data-testid="file-upload-input"
                  onChange={(e) => {
                    if (e.target.files && e.target.files.length > 0) {
                      processImageFile(e.target.files[0]);
                      e.target.value = '';
                    }
                  }}
                />
                <Upload className={`w-6 h-6 text-slate-400 dark:text-zinc-500 mx-auto mb-2 ${isProcessing ? 'animate-bounce text-indigo-500' : ''}`} />
                <p className="text-xs font-medium text-slate-700 dark:text-zinc-300">
                  {isProcessing ? 'Processing QR payload...' : (
                    <>
                      Drag and drop a QR code image here, or <span className="text-indigo-600 dark:text-indigo-400 underline font-semibold">browse</span>
                    </>
                  )}
                </p>
                <p className="text-xs text-slate-500 dark:text-zinc-400 mt-1">Supports PNG, JPG, WebP, or pairing JSON fixture</p>
              </div>
            </div>
          )}
        </div>

        {/* Right Column: Guide & Diagnostic Details (5 Cols) */}
        <div className="lg:col-span-5 flex flex-col justify-between space-y-4">
          <div className="glass-panel rounded-2xl p-6 shadow-xs">
            <div className="flex items-center space-x-2.5 mb-3">
              <div className="p-1.5 rounded-lg bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/40">
                <HelpCircle className="w-4 h-4" />
              </div>
              <h3 className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100">Pairing Instructions</h3>
            </div>
            <p className="text-xs text-slate-600 dark:text-zinc-400 leading-relaxed mb-4">
              To control this Android device from your desktop development environment:
            </p>
            <ol className="text-xs text-slate-600 dark:text-zinc-300 space-y-2.5 pl-4 list-decimal">
              <li>Run <code className="px-1.5 py-0.5 rounded bg-slate-100 dark:bg-zinc-800 font-mono text-indigo-600 dark:text-indigo-400 font-semibold">texure pair</code> in your terminal.</li>
              <li>Point this camera at the displayed ASCII/PNG QR code.</li>
              <li>The reverse tunnel establishes an encrypted TLS channel automatically.</li>
            </ol>

            <div className="mt-5 pt-4 border-t border-slate-100 dark:border-zinc-800/60 space-y-2">
              <div className="flex justify-between text-xs text-slate-600 dark:text-zinc-400">
                <span>Security</span>
                <span className="font-mono text-slate-800 dark:text-zinc-200 font-semibold">WSS + Bearer Token</span>
              </div>
              <div className="flex justify-between text-xs text-slate-600 dark:text-zinc-400">
                <span>Protocol Spec</span>
                <span className="font-mono text-slate-800 dark:text-zinc-200 font-semibold">Standard WSS Protocol</span>
              </div>
            </div>
          </div>

          {/* Test Aid Card */}
          {isBrowserMock && (
            <div className="glass-panel rounded-2xl p-5 border-dashed border-indigo-400 dark:border-indigo-500 shadow-xs">
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100">Development Shortcut</span>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 border border-indigo-200/60 dark:border-indigo-800/40">
                  SIMULATION
                </span>
              </div>
              <p className="text-xs text-slate-600 dark:text-zinc-400 mb-3">
                Click below to instantly feed a verified sample pairing payload without a physical camera:
              </p>
              <button
                data-testid="load-fixture-qr-btn"
                onClick={() => handlePayloadResult(BB_R11_TEST_PAYLOAD)}
                className="w-full px-3.5 py-2.5 rounded-xl bg-indigo-50 hover:bg-indigo-100 dark:bg-indigo-950/60 dark:hover:bg-indigo-900/60 border border-indigo-500 hover:border-indigo-600 dark:border-indigo-600 text-indigo-700 dark:text-indigo-300 text-xs font-sans font-semibold transition shadow-xs text-center active:scale-[0.99]"
              >
                Load Sample Pairing Payload
              </button>
            </div>
          )}

          {/* Subnet Discovery Spec */}
          <div className="glass-panel rounded-2xl p-5 shadow-xs space-y-2">
            <div className="flex items-center space-x-2">
              <div className="p-1 rounded-lg bg-emerald-50 dark:bg-emerald-950/60 text-emerald-600 dark:text-emerald-400 border border-emerald-200/60 dark:border-emerald-800/40">
                <Radio className="w-3.5 h-3.5" />
              </div>
              <span className="text-sm font-semibold tracking-tight text-slate-900 dark:text-zinc-100">Zero-Conf Subnet Beacon</span>
            </div>
            <p className="text-xs text-slate-600 dark:text-zinc-400 leading-relaxed">
              Companion broadcasts mDNS beacon service <code className="px-1.5 py-0.5 rounded bg-slate-100 dark:bg-zinc-800/80 font-mono text-indigo-600 dark:text-indigo-400 text-[11px]">_texure-bridge._tcp.local</code> for automatic LAN discovery.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};
