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
      return 'bg-sky-500/15 text-sky-400 border-sky-500/30';
    case 'WARN':
      return 'bg-amber-500/15 text-amber-400 border-amber-500/30';
    case 'ERROR':
      return 'bg-rose-500/15 text-rose-400 border-rose-500/30';
    case 'DEBUG':
      return 'bg-purple-500/15 text-purple-400 border-purple-500/30';
    default:
      return 'bg-zinc-500/15 text-zinc-400 border-zinc-700/40';
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
  const rowTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isProgrammaticScrollRef = useRef<boolean>(false);

  useEffect(() => {
    return () => {
      if (copyTimeoutRef.current) {
        clearTimeout(copyTimeoutRef.current);
      }
      if (rowTimeoutRef.current) {
        clearTimeout(rowTimeoutRef.current);
      }
    };
  }, []);

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

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      isProgrammaticScrollRef.current = true;
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
      const rId = requestAnimationFrame(() => {
        isProgrammaticScrollRef.current = false;
      });
      return () => cancelAnimationFrame(rId);
    }
  }, [filteredLogs, autoScroll]);

  const handleScroll = useCallback(() => {
    if (isProgrammaticScrollRef.current || !containerRef.current) return;
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
      if (rowTimeoutRef.current) clearTimeout(rowTimeoutRef.current);
      rowTimeoutRef.current = setTimeout(() => setCopiedRowId(null), 1500);
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
    <div data-testid="log-viewer" className="space-y-6">
      {/* Top Header & Metrics */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-xl font-semibold tracking-tight text-slate-900 dark:text-zinc-100 flex items-center space-x-2">
            <Terminal className="w-5 h-5 text-indigo-500 dark:text-indigo-400" />
            <span>Real-time Event Log Viewer</span>
          </h2>
          <p className="text-xs text-slate-600 dark:text-zinc-400 mt-1">
            Streaming IPC, reverse tunnel events, and Shizuku lifecycle activity (capped to 500 lines).
          </p>
        </div>

        <div className="flex items-center space-x-2 text-xs font-mono bg-white dark:bg-zinc-900 px-3.5 py-1.5 rounded-xl border border-slate-300 dark:border-zinc-800 shadow-xs self-start sm:self-auto">
          <span className="text-slate-500 dark:text-zinc-400">Showing</span>
          <span className="text-indigo-600 dark:text-indigo-400 font-semibold">{filteredLogs.length}</span>
          <span className="text-slate-300 dark:text-zinc-600">/</span>
          <span className="text-slate-800 dark:text-zinc-200 font-semibold">{logs.length}</span>
          <span className="text-slate-500 dark:text-zinc-400">entries</span>
        </div>
      </div>

      {/* Control Toolbar */}
      <div className="glass-panel rounded-2xl p-3.5 flex flex-wrap items-center justify-between gap-3 shadow-xs">
        {/* Left: Search & Level Filter */}
        <div className="flex flex-wrap items-center gap-2.5 flex-1 min-w-[240px]">
          {/* Search Box */}
          <div className="relative flex-1 min-w-[160px] max-w-sm">
            <Search className="w-4 h-4 text-slate-400 dark:text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search logs or sources..."
              data-testid="input-log-search"
              className="w-full pl-9 pr-8 py-1.5 bg-slate-50 dark:bg-zinc-950 border border-slate-300 dark:border-zinc-700 rounded-xl text-xs font-mono text-slate-900 dark:text-zinc-100 placeholder-slate-500 dark:placeholder-zinc-500 focus:outline-none focus:border-indigo-500 dark:focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500 transition-all"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-700 dark:hover:text-zinc-200"
                aria-label="Clear search"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          {/* Level Filter Dropdown */}
          <div className="flex items-center space-x-1.5 bg-slate-50 dark:bg-zinc-950 px-2.5 py-1.5 rounded-xl border border-slate-300 dark:border-zinc-700">
            <Filter className="w-3.5 h-3.5 text-slate-400 dark:text-zinc-500" />
            <select
              value={selectedLevel}
              onChange={(e) => setSelectedLevel(e.target.value as FilterLevel)}
              data-testid="select-log-level"
              className="bg-transparent text-xs text-slate-700 dark:text-zinc-200 font-medium focus:outline-none cursor-pointer"
            >
              <option value="ALL" className="bg-white dark:bg-zinc-900 text-slate-900 dark:text-zinc-100">All Levels</option>
              <option value="INFO" className="bg-white dark:bg-zinc-900 text-sky-600 dark:text-sky-400">INFO</option>
              <option value="WARN" className="bg-white dark:bg-zinc-900 text-amber-600 dark:text-amber-400">WARN</option>
              <option value="ERROR" className="bg-white dark:bg-zinc-900 text-rose-600 dark:text-rose-400">ERROR</option>
              <option value="DEBUG" className="bg-white dark:bg-zinc-900 text-purple-600 dark:text-purple-400">DEBUG</option>
            </select>
          </div>
        </div>

        {/* Right: Actions */}
        <div className="flex items-center space-x-2">
          {copyError && (
            <span className="text-xs text-rose-700 dark:text-rose-400 flex items-center space-x-1">
              <AlertCircle className="w-3.5 h-3.5" />
              <span>{copyError}</span>
            </span>
          )}

          {/* Auto Scroll Toggle */}
          <button
            onClick={() => setAutoScroll((prev) => !prev)}
            data-testid="btn-autoscroll-toggle"
            title={autoScroll ? 'Auto-scroll enabled' : 'Auto-scroll paused'}
            className={`inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-xl text-xs font-medium border transition-all shadow-xs ${
              autoScroll
                ? 'bg-indigo-50 dark:bg-indigo-950/60 text-indigo-700 dark:text-indigo-400 border-indigo-500 dark:border-indigo-600 font-semibold'
                : 'bg-slate-100 hover:bg-slate-200/90 text-slate-800 dark:bg-zinc-800 dark:text-zinc-200 border-slate-300 dark:border-zinc-600'
            }`}
          >
            {autoScroll ? (
              <ArrowDownCircle className="w-3.5 h-3.5 text-indigo-600" />
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
            className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-slate-100 hover:bg-slate-200/90 dark:bg-zinc-800 hover:dark:bg-zinc-700/80 disabled:opacity-50 text-slate-800 dark:text-zinc-200 text-xs font-medium border border-slate-300 dark:border-zinc-600 transition shadow-xs"
          >
            {copied ? (
              <Check className="w-3.5 h-3.5 text-emerald-500" />
            ) : (
              <Copy className="w-3.5 h-3.5 text-slate-500" />
            )}
            <span className="hidden sm:inline">{copied ? 'Copied' : 'Copy'}</span>
          </button>

          {/* Export File */}
          <button
            onClick={handleExport}
            disabled={filteredLogs.length === 0}
            data-testid="btn-export-logs"
            className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-slate-100 hover:bg-slate-200/90 dark:bg-zinc-800 hover:dark:bg-zinc-700/80 disabled:opacity-50 text-slate-800 dark:text-zinc-200 text-xs font-medium border border-slate-300 dark:border-zinc-600 transition shadow-xs"
          >
            <Download className="w-3.5 h-3.5 text-slate-500" />
            <span className="hidden sm:inline">Export</span>
          </button>

          {/* Clear Logs */}
          <button
            onClick={clearLogs}
            disabled={logs.length === 0}
            data-testid="btn-clear-logs"
            className="inline-flex items-center space-x-1.5 px-3 py-1.5 rounded-xl bg-rose-50 hover:bg-rose-100 dark:bg-rose-500/10 dark:hover:bg-rose-500/20 disabled:opacity-50 text-rose-700 dark:text-rose-400 text-xs font-medium border border-rose-500 dark:border-rose-500/40 transition shadow-xs"
          >
            <Trash2 className="w-3.5 h-3.5 text-rose-600 dark:text-rose-400" />
            <span className="hidden sm:inline">Clear</span>
          </button>
        </div>
      </div>

      {/* Terminal Viewport - Raised Metal Frame with Inset Log Engine */}
      <div className="bg-[#1e1e24] dark:bg-[#1e1e24] border border-slate-300 dark:border-white/20 rounded-2xl shadow-xl overflow-hidden flex flex-col">
        {/* Terminal Header Bar */}
        <div className="px-4 py-2.5 bg-[#141418] border-b border-zinc-800 flex items-center justify-between text-xs text-zinc-400 font-mono">
          <div className="flex items-center space-x-2">
            <span className="w-3 h-3 rounded-full bg-rose-500/80 inline-block" />
            <span className="w-3 h-3 rounded-full bg-amber-500/80 inline-block" />
            <span className="w-3 h-3 rounded-full bg-emerald-500/80 inline-block" />
            <span className="text-xs text-zinc-300 ml-2 font-sans font-medium">texure-stream.log</span>
          </div>
          <span className="text-xs text-zinc-400">UTF-8 • STREAMING</span>
        </div>

        {/* Scrollable Container */}
        <div
          ref={containerRef}
          onScroll={handleScroll}
          data-testid="log-stream-container"
          className="bg-[#101013] flex-1 min-h-[140px] max-h-[520px] p-4 overflow-y-auto font-mono text-xs leading-relaxed text-zinc-300 select-text flex flex-col"
        >
          {filteredLogs.length === 0 ? (
            <div data-testid="log-empty-state" className="flex-1 flex flex-col items-center justify-center text-zinc-500 space-y-2.5 py-4">
              <div className="w-12 h-12 rounded-2xl bg-zinc-900 border border-zinc-800 flex items-center justify-center text-zinc-500">
                <Terminal className="w-6 h-6 opacity-60" />
              </div>
              <p className="text-xs">No event log entries found matching criteria.</p>
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery('')}
                  className="text-xs text-indigo-400 hover:underline font-sans"
                >
                  Clear search query
                </button>
              )}
            </div>
          ) : (
            <div className="space-y-1">
              {filteredLogs.map((entry) => (
                <div
                  key={entry.id}
                  data-testid="log-entry-row"
                  className="group flex items-start space-x-2.5 hover:bg-zinc-900/60 p-1 rounded-lg transition duration-150"
                >
                  {/* Timestamp */}
                  <span className="text-zinc-400 shrink-0 select-none text-xs">
                    {entry.timestamp}
                  </span>

                  {/* Level Badge */}
                  <span
                    data-testid="log-level-badge"
                    className={`inline-block px-1.5 py-0.5 rounded text-[11px] font-semibold border shrink-0 ${getLevelBadgeStyles(
                      entry.level
                    )}`}
                  >
                    {entry.level}
                  </span>

                  {/* Source - fixed width for vertical columnar alignment */}
                  <span className="w-24 shrink-0 font-mono text-xs text-indigo-400/90 dark:text-indigo-400/80 font-normal truncate">
                    {entry.source ? `[${entry.source}]` : ''}
                  </span>

                  {/* Message with token-preserving word wrapping */}
                  <span data-testid="log-message-text" className="flex-1 break-words whitespace-pre-wrap text-zinc-200">
                    {entry.message}
                  </span>

                  {/* Line Copy Action */}
                  <button
                    onClick={() => handleCopyRow(entry)}
                    title="Copy log entry"
                    aria-label="Copy log entry"
                    className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100 transition text-zinc-400 hover:text-zinc-200 focus:outline-none focus:ring-1 focus:ring-indigo-400 shrink-0 p-1 rounded hover:bg-zinc-800"
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
    </div>
  );
};
