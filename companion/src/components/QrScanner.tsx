import React, { useState, useRef, useEffect, useCallback } from 'react';
import { Camera, Upload, AlertCircle, CheckCircle2, RefreshCw, X, ShieldCheck } from 'lucide-react';
import { PairingConfig } from '../types';
import { parsePairingPayload } from '../lib/qr-parser';
import { useCompanion } from '../context/CompanionContext';

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

      setSuccessConfig(res.config);
      await pairDevice(res.config);
      if (onPairSuccess) {
        onPairSuccess(res.config);
      }
      setIsProcessing(false);
      stopCamera();
    },
    [pairDevice, onPairSuccess]
  );

  const stopCamera = useCallback(() => {
    if (animFrameRef.current) {
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
    setScanning(false);
  }, []);

  const scanFrame = useCallback(async () => {
    if (!videoRef.current || !scanning || successConfig) return;

    const video = videoRef.current;
    if (video.readyState === video.HAVE_ENOUGH_DATA) {
      // 1. Check BarcodeDetector API
      if ('BarcodeDetector' in window) {
        try {
          const barcodeDetector = new (window as any).BarcodeDetector({ formats: ['qr_code'] });
          const barcodes = await barcodeDetector.detect(video);
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
        throw new Error('Camera device access is not supported in this environment');
      }

      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment', width: { ideal: 640 }, height: { ideal: 480 } },
      });

      streamRef.current = stream;
      if (videoRef.current) {
        videoRef.current.srcObject = stream;
        await videoRef.current.play();
        setScanning(true);
      }
    } catch (err: any) {
      console.warn('Camera start error:', err);
      setCameraError(err.message || 'Unable to access camera. Please use file upload fallback.');
      setScanning(false);
    }
  }, [stopCamera]);

  useEffect(() => {
    if (scanning) {
      animFrameRef.current = requestAnimationFrame(scanFrame);
    }
    return () => {
      stopCamera();
    };
  }, [scanning, scanFrame, stopCamera]);

  // Handle uploaded image file
  const processImageFile = async (file: File) => {
    setIsProcessing(true);
    setParseError(null);

    try {
      // Check for BarcodeDetector on ImageBitmap
      if ('BarcodeDetector' in window) {
        const imageBitmap = await createImageBitmap(file);
        const detector = new (window as any).BarcodeDetector({ formats: ['qr_code'] });
        const barcodes = await detector.detect(imageBitmap);
        if (barcodes.length > 0 && barcodes[0].rawValue) {
          await handlePayloadResult(barcodes[0].rawValue);
          return;
        }
      }

      // Fallback: Read using ZXing
      try {
        const { BrowserQRCodeReader } = await import('@zxing/browser');
        const codeReader = new BrowserQRCodeReader();
        const imgUrl = URL.createObjectURL(file);
        const result = await codeReader.decodeFromImageUrl(imgUrl);
        URL.revokeObjectURL(imgUrl);

        if (result && result.getText()) {
          await handlePayloadResult(result.getText());
          return;
        }
      } catch (zxingErr) {
        console.debug('ZXing decoding failed:', zxingErr);
      }

      // If text file (for raw JSON fixtures)
      if (file.type === 'application/json' || file.name.endsWith('.json') || file.type.startsWith('text/')) {
        const text = await readFileAsText(file);
        await handlePayloadResult(text);
        return;
      }

      setParseError('No QR code detected in the selected image. Please try another file.');
    } catch (err: any) {
      setParseError(`Failed to process image: ${err.message || err}`);
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
    <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden max-w-xl mx-auto shadow-2xl">
      {/* Header */}
      <div className="px-5 py-3 border-b border-slate-800 flex items-center justify-between bg-slate-900/80">
        <div className="flex items-center space-x-2">
          <Camera className="w-4 h-4 text-indigo-400" />
          <h2 className="text-sm font-semibold text-slate-100">Pair Remote Gateway</h2>
        </div>
        {onCancel && (
          <button onClick={onCancel} className="text-slate-400 hover:text-slate-200">
            <X className="w-4 h-4" />
          </button>
        )}
      </div>

      <div className="p-6 space-y-6">
        {/* Success View */}
        {successConfig ? (
          <div data-testid="pair-success-state" className="text-center py-6 space-y-4">
            <div className="w-16 h-16 rounded-full bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center mx-auto text-emerald-400">
              <CheckCircle2 className="w-8 h-8" />
            </div>
            <div>
              <h3 className="text-base font-medium text-emerald-400">Pairing Successful!</h3>
              <p className="text-xs text-slate-400 mt-1">
                Connected to endpoint: <span className="font-mono text-slate-300">{successConfig.url}</span>
              </p>
            </div>
            <div className="inline-flex items-center space-x-2 text-xs bg-slate-800 px-3 py-1.5 rounded-lg border border-slate-700">
              <ShieldCheck className="w-3.5 h-3.5 text-indigo-400" />
              <span className="text-slate-300 font-mono">Token: {successConfig.token.substring(0, 12)}...</span>
            </div>
            <div>
              <button
                onClick={() => {
                  setSuccessConfig(null);
                  startCamera();
                }}
                className="text-xs text-indigo-400 hover:text-indigo-300 underline"
              >
                Scan Another QR
              </button>
            </div>
          </div>
        ) : (
          <>
            {/* Viewfinder or Camera Feed */}
            <div className="relative aspect-video rounded-lg overflow-hidden bg-slate-950 border border-slate-800 flex flex-col items-center justify-center">
              {scanning ? (
                <>
                  <video
                    ref={videoRef}
                    playsInline
                    muted
                    className="w-full h-full object-cover"
                  />
                  {/* Viewfinder overlay */}
                  <div className="absolute inset-0 border-2 border-indigo-500/40 pointer-events-none flex items-center justify-center">
                    <div className="w-48 h-48 border-2 border-indigo-400/80 rounded-lg relative">
                      <div className="absolute top-0 left-0 w-4 h-4 border-t-2 border-l-2 border-indigo-400 -mt-0.5 -ml-0.5" />
                      <div className="absolute top-0 right-0 w-4 h-4 border-t-2 border-r-2 border-indigo-400 -mt-0.5 -mr-0.5" />
                      <div className="absolute bottom-0 left-0 w-4 h-4 border-b-2 border-l-2 border-indigo-400 -mb-0.5 -ml-0.5" />
                      <div className="absolute bottom-0 right-0 w-4 h-4 border-b-2 border-r-2 border-indigo-400 -mb-0.5 -mr-0.5" />
                      <div className="w-full h-0.5 bg-indigo-500 animate-pulse mt-24 shadow-lg shadow-indigo-500" />
                    </div>
                  </div>
                </>
              ) : (
                <div className="text-center p-6 space-y-3">
                  <div className="w-12 h-12 rounded-full bg-slate-800 flex items-center justify-center mx-auto text-slate-400">
                    <Camera className="w-6 h-6" />
                  </div>
                  <div>
                    <p className="text-xs text-slate-400">Camera preview is currently inactive.</p>
                  </div>
                  <button
                    onClick={startCamera}
                    className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-medium transition inline-flex items-center space-x-1.5 shadow"
                  >
                    <RefreshCw className="w-3.5 h-3.5" />
                    <span>Start Camera Scan</span>
                  </button>
                </div>
              )}

              {scanning && (
                <button
                  onClick={stopCamera}
                  className="absolute top-2 right-2 p-1.5 bg-slate-900/80 hover:bg-slate-900 rounded-md text-slate-300 text-xs border border-slate-700"
                >
                  Stop Camera
                </button>
              )}
            </div>

            {/* Error notifications */}
            {cameraError && (
              <div className="p-3 bg-amber-950/40 border border-amber-600/30 rounded-lg text-xs text-amber-300 flex items-start space-x-2">
                <AlertCircle className="w-4 h-4 shrink-0 text-amber-400 mt-0.5" />
                <span>{cameraError}</span>
              </div>
            )}

            {parseError && (
              <div data-testid="parse-error-message" className="p-3 bg-rose-950/40 border border-rose-600/30 rounded-lg text-xs text-rose-300 flex items-start space-x-2">
                <AlertCircle className="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
                <span>{parseError}</span>
              </div>
            )}

            {/* Drag & drop / File Upload Fallback */}
            <div
              onDragOver={(e) => {
                e.preventDefault();
                setDragActive(true);
              }}
              onDragLeave={() => setDragActive(false)}
              onDrop={handleDrop}
              onClick={() => fileInputRef.current?.click()}
              className={`border-2 border-dashed rounded-lg p-5 text-center cursor-pointer transition ${
                dragActive
                  ? 'border-indigo-400 bg-indigo-950/20'
                  : 'border-slate-800 hover:border-slate-700 bg-slate-950/40'
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
                  }
                }}
              />
              <Upload className={`w-6 h-6 text-slate-500 mx-auto mb-2 ${isProcessing ? 'animate-bounce text-indigo-400' : ''}`} />
              <p className="text-xs font-medium text-slate-300">
                {isProcessing ? 'Processing QR payload...' : (
                  <>
                    Drag and drop a QR code image here, or <span className="text-indigo-400 underline">browse</span>
                  </>
                )}
              </p>
              <p className="text-[10px] text-slate-500 mt-1">Supports PNG, JPG, WebP, or pairing JSON fixture</p>
            </div>

            {/* One-click fixture tester (for browser mock & headless testing) */}
            {isBrowserMock && (
              <div className="pt-2 border-t border-slate-800 flex justify-between items-center text-xs">
                <span className="text-slate-500 text-[11px]">Desktop & Mock Test Aid:</span>
                <button
                  data-testid="load-fixture-qr-btn"
                  onClick={() =>
                    handlePayloadResult(
                      JSON.stringify({
                        scheme: 'wss',
                        host: 'desktop.example',
                        port: 443,
                        path: '/tunnel',
                        token: 'mock-bearer-token-bb-r11',
                        app: 'texure',
                      })
                    )
                  }
                  className="px-2.5 py-1 rounded bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-300 text-[11px]"
                >
                  Load Golden Pairing Payload (BB-R11)
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};
