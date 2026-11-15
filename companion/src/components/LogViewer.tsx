import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { useCompanion } from '../context/CompanionContext';
import { LogLevel, LogEntry } from '../types';
import {
  Terminal,
  Search,
  Trash2,
  Download,
  Copy,
  Check,
  ArrowDownCircle,
  PauseCircle,
  Filter,
  X,
  AlertCircle,
} from 'lucide-react';

type FilterLevel = LogLevel | 'ALL';

const getLevelBadgeStyles = (level: LogLevel): string => {
  switch (level) {
    case 'INFO':
      return 'bg-sky-500/10 text-sky-400 border-sky-500/30';
    case 'WARN':
      return 'bg-amber-500/10 text-amber-400 border-amber-500/30';
    case 'ERROR':
      return 'bg-rose-500/10 text-rose-400 border-rose-500/30';
    case 'DEBUG':
      return 'bg-purple-500/10 text-purple-400 border-purple-500/30';
  }
};

const formatLogLine = (entry: LogEntry): string => {
  const source = entry.source ? ` [${entry.source}]` : '';
  return `[${entry.timestamp}] [${entry.level}]${source} ${entry.message}`;
};

export const LogViewer: React.FC = () => {
  const { logs, clearLogs, exportLogs } = useCompanion();

  const [searchQuery, setSearchQuery] = useState<string>('');
  const [selectedLevel, setSelectedLevel] = useState<FilterLevel>('ALL');
  const [autoScroll, setAutoScroll] = useState<boolean>(true);
  const [copied, setCopied] = useState<boolean>(false);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copiedRowId, setCopiedRowId] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (copyTimeoutRef.current) {
        clearTimeout(copyTimeoutRef.current);
      }
    };
  }, []);

  // Filter logs according to level and query
  const filteredLogs = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    return logs.filter((log) => {
      const matchesLevel = selectedLevel === 'ALL' || log.level === selectedLevel;
      if (!matchesLevel) return false;

      if (!query) return true;
      const msgMatch = log.message.toLowerCase().includes(query);
      const srcMatch = log.source ? log.source.toLowerCase().includes(query) : false;
      const levelMatch = log.level.toLowerCase().includes(query);
      return msgMatch || srcMatch || levelMatch;
    });
  }, [logs, selectedLevel, searchQuery]);

  // Auto-scroll when new logs arrive if enabled
  useEffect(() => {
    if (
      autoScroll &&
      containerRef.current &&
      endRef.current &&
      typeof endRef.current.scrollIntoView === 'function'
    ) {
      endRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [filteredLogs, autoScroll]);

  // Detect user scroll up to toggle auto-scroll off
  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const atBottom = scrollHeight - scrollTop - clientHeight < 40;
    if (!atBottom && autoScroll) {
      setAutoScroll(false);
    } else if (atBottom && !autoScroll) {
      setAutoScroll(true);
    }
  }, [autoScroll]);

  const copyToClipboard = useCallback(async (text: string): Promise<boolean> => {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
        setCopyError(null);
        return true;
      }
      throw new Error('Clipboard API not available');
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Clipboard operation failed';
      console.warn('Clipboard write failed:', err);
      setCopyError(msg);
      if (copyTimeoutRef.current) clearTimeout(copyTimeoutRef.current);
      copyTimeoutRef.current = setTimeout(() => setCopyError(null), 3000);
      return false;
    }
  }, []);

  const handleCopyAll = useCallback(async () => {
    const text = filteredLogs.map(formatLogLine).join('\n');
    const ok = await copyToClipboard(text);
    if (ok) {
      setCopied(true);
      if (copyTimeoutRef.current) clearTimeout(copyTimeoutRef.current);
      copyTimeoutRef.current = setTimeout(() => setCopied(false), 2000);
    }
  }, [filteredLogs, copyToClipboard]);

  const handleCopyRow = useCallback(async (entry: LogEntry) => {
    const text = formatLogLine(entry);
    const ok = await copyToClipboard(text);
    if (ok) {
      setCopiedRowId(entry.id);
      setTimeout(() => setCopiedRowId(null), 1500);
    }
  }, [copyToClipboard]);

  const handleExport = useCallback(async () => {
    try {
      let exportText = '';
      if (filteredLogs.length === logs.length) {
        exportText = await exportLogs();
      } else {
        exportText = filteredLogs.map(formatLogLine).join('\n');
      }

      if (typeof window !== 'undefined' && typeof document !== 'undefined') {
        const blob = new Blob([exportText], { type: 'text/plain;charset=utf-8' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `texure-logs-${new Date().toISOString().replace(/[:.]/g, '-')}.txt`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
      }
    } catch (err) {
      console.error('Failed to export logs:', err);
    }
  }, [filteredLogs, logs.length, exportLogs]);

  return (
    <div data-testid="log-viewer" className="space-y-4">
      {/* Top Header & Metrics */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-bold tracking-tight text-slate-100 flex items-center space-x-2">
            <Terminal className="w-5 h-5 text-indigo-400" />
            <span>Real-time Event Log Viewer</span>
          </h2>
          <p className="text-xs text-slate-400 mt-1">
            Streaming IPC, reverse tunnel events, and Shizuku lifecycle activity (capped to 500 lines).
          </p>
        </div>

        <div className="flex items-center space-x-2 text-xs font-mono text-slate-400 bg-slate-900 px-3 py-1.5 rounded-lg border border-slate-800 self-start sm:self-auto">
          <span className="text-slate-500">Showing</span>
          <span className="text-indigo-400 font-semibold">{filteredLogs.length}</span>
          <span className="text-slate-600">/</span>
          <span className="text-slate-300">{logs.length}</span>
          <span className="text-slate-500">entries</span>
        </div>
      </div>

      {/* Control Toolbar */}
      <div className="bg-slate-900/90 border border-slate-800 rounded-xl p-3 flex flex-wrap items-center justify-between gap-3 shadow-lg">
        {/* Left: Search & Level Filter */}
        <div className="flex flex-wrap items-center gap-2 flex-1 min-w-[240px]">
          {/* Search Box */}
          <div className="relative flex-1 min-w-[160px] max-w-sm">
            <Search className="w-4 h-4 text-slate-500 absolute left-2.5 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search logs or sources..."
              data-testid="input-log-search"
              className="w-full pl-8 pr-7 py-1.5 bg-slate-950 border border-slate-800 rounded-lg text-xs font-mono text-slate-200 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-500 hover:text-slate-300"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          {/* Level Filter Dropdown */}
          <div className="flex items-center space-x-1.5 bg-slate-950 px-2 py-1 rounded-lg border border-slate-800">
            <Filter className="w-3.5 h-3.5 text-slate-500" />
            <select
              value={selectedLevel}
              onChange={(e) => setSelectedLevel(e.target.value as FilterLevel)}
              data-testid="select-log-level"
              className="bg-transparent text-xs text-slate-300 font-medium focus:outline-none cursor-pointer"
            >
              <option value="ALL" className="bg-slate-900 text-slate-200">All Levels</option>
              <option value="INFO" className="bg-slate-900 text-sky-400">INFO</option>
              <option value="WARN" className="bg-slate-900 text-amber-400">WARN</option>
              <option value="ERROR" className="bg-slate-900 text-rose-400">ERROR</option>
              <option value="DEBUG" className="bg-slate-900 text-purple-400">DEBUG</option>
            </select>
          </div>
        </div>

        {/* Right: Actions */}
        <div className="flex items-center space-x-2">
          {copyError && (
            <span className="text-[11px] text-rose-400 flex items-center space-x-1">
              <AlertCircle className="w-3.5 h-3.5" />
              <span>{copyError}</span>
            </span>
          )}

          {/* Auto Scroll Toggle */}
          <button
            onClick={() => setAutoScroll((prev) => !prev)}
            data-testid="btn-autoscroll-toggle"
            title={autoScroll ? 'Auto-scroll enabled' : 'Auto-scroll paused'}
            className={`inline-flex items-center space-x-1 px-2.5 py-1.5 rounded-lg text-xs font-medium border transition ${
              autoScroll
                ? 'bg-indigo-600/20 text-indigo-300 border-indigo-500/30 hover:bg-indigo-600/30'
                : 'bg-slate-800 text-slate-400 border-slate-700 hover:bg-slate-700'
            }`}
          >
            {autoScroll ? (
              <ArrowDownCircle className="w-3.5 h-3.5 text-indigo-400" />
            ) : (
              <PauseCircle className="w-3.5 h-3.5 text-slate-500" />
            )}
            <span className="hidden sm:inline">{autoScroll ? 'Auto-scroll ON' : 'Paused'}</span>
          </button>

          {/* Copy All */}
          <button
            onClick={handleCopyAll}
            disabled={filteredLogs.length === 0}
            data-testid="btn-copy-logs"
            className="inline-flex items-center space-x-1 px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-slate-200 text-xs font-medium border border-slate-700 transition"
          >
            {copied ? (
              <Check className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <Copy className="w-3.5 h-3.5 text-slate-400" />
            )}
            <span className="hidden sm:inline">{copied ? 'Copied' : 'Copy'}</span>
          </button>

          {/* Export File */}
          <button
            onClick={handleExport}
            disabled={filteredLogs.length === 0}
            data-testid="btn-export-logs"
            className="inline-flex items-center space-x-1 px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 disabled:opacity-50 text-slate-200 text-xs font-medium border border-slate-700 transition"
          >
            <Download className="w-3.5 h-3.5 text-slate-400" />
            <span className="hidden sm:inline">Export</span>
          </button>

          {/* Clear Logs */}
          <button
            onClick={clearLogs}
            disabled={logs.length === 0}
            data-testid="btn-clear-logs"
            className="inline-flex items-center space-x-1 px-2.5 py-1.5 rounded-lg bg-rose-600/10 hover:bg-rose-600/20 disabled:opacity-50 text-rose-400 text-xs font-medium border border-rose-500/30 transition"
          >
            <Trash2 className="w-3.5 h-3.5 text-rose-400" />
            <span className="hidden sm:inline">Clear</span>
          </button>
        </div>
      </div>

      {/* Terminal Viewport */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        data-testid="log-stream-container"
        className="h-[520px] bg-slate-950 border border-slate-800 rounded-xl p-4 overflow-y-auto font-mono text-xs leading-relaxed text-slate-300 shadow-2xl select-text"
      >
        {filteredLogs.length === 0 ? (
          <div data-testid="log-empty-state" className="h-full flex flex-col items-center justify-center text-slate-500 space-y-2">
            <Terminal className="w-8 h-8 opacity-40 text-slate-400" />
            <p className="text-xs">No event log entries found matching criteria.</p>
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="text-[11px] text-indigo-400 hover:underline"
              >
                Clear search query
              </button>
            )}
          </div>
        ) : (
          <div className="space-y-1.5">
            {filteredLogs.map((entry) => (
              <div
                key={entry.id}
                data-testid="log-entry-row"
                className="group flex items-start space-x-2.5 hover:bg-slate-900/60 p-1 rounded transition duration-150"
              >
                {/* Timestamp */}
                <span className="text-slate-500 shrink-0 select-none text-[11px]">
                  {entry.timestamp}
                </span>

                {/* Level Chip */}
                <span
                  data-testid="log-level-badge"
                  className={`inline-block px-1.5 py-0.2 rounded text-[10px] font-semibold border shrink-0 ${getLevelBadgeStyles(
                    entry.level
                  )}`}
                >
                  {entry.level}
                </span>

                {/* Source if present */}
                {entry.source && (
                  <span className="text-indigo-400 shrink-0 text-[11px]">
                    [{entry.source}]
                  </span>
                )}

                {/* Message */}
                <span data-testid="log-message-text" className="flex-1 break-all text-slate-200">
                  {entry.message}
                </span>

                {/* Individual Line Copy Button */}
                <button
                  onClick={() => handleCopyRow(entry)}
                  title="Copy log entry"
                  className="opacity-0 group-hover:opacity-100 transition text-slate-500 hover:text-slate-300 shrink-0 p-0.5"
                >
                  {copiedRowId === entry.id ? (
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                  ) : (
                    <Copy className="w-3.5 h-3.5" />
                  )}
                </button>
              </div>
            ))}
            <div ref={endRef} />
          </div>
        )}
      </div>
    </div>
  );
};
