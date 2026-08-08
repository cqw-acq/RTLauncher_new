"use client";

import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { AnimatePresence, motion } from "framer-motion";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { ModDependencyPanel } from "@/components/launch/mod-dependency-panel";
import {
  Terminal,
  Trash2,
  Maximize2,
  Download,
  Copy,
  Search,
  Pencil,
  Square,
  Puzzle,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/components/i18n/use-i18n";

type ConsoleTab = "logs" | "analysis";

const LOG_LEVELS = ["fatal", "error", "warn", "info", "debug"] as const;
type LogLevel = (typeof LOG_LEVELS)[number];

const levelColors: Record<string, string> = {
  fatal: "text-red-600 dark:text-red-400",
  error: "text-destructive",
  warn: "text-amber-500",
  info: "text-muted-foreground",
  debug: "text-sky-500",
};

const levelBgColors: Record<string, string> = {
  fatal: "bg-red-500/10 border-red-500/30",
  error: "bg-destructive/10 border-destructive/30",
  warn: "bg-amber-500/10 border-amber-500/30",
  info: "bg-muted/50 border-border/30",
  debug: "bg-sky-500/10 border-sky-500/30",
};

const levelDotColors: Record<string, string> = {
  fatal: "bg-red-500",
  error: "bg-destructive",
  warn: "bg-amber-500",
  info: "bg-muted-foreground/60",
  debug: "bg-sky-500",
};

function autoGuessLevel(message: string): LogLevel {
  const lower = message.toLowerCase();
  if (lower.includes("fatal") || lower.includes("overwriting existing")) return "fatal";
  if (lower.includes("error") || lower.includes("exception") || lower.includes("stacktrace")) return "error";
  if (lower.includes("warn") || lower.includes("warning")) return "warn";
  if (lower.includes("debug") || lower.includes("trace")) return "debug";
  return "info";
}

interface LogLine {
  id: number;
  timestamp: string;
  level: LogLevel;
  message: string;
}

function countLevels(logs: LogLine[]) {
  const counts: Record<LogLevel, number> = { fatal: 0, error: 0, warn: 0, info: 0, debug: 0 };
  for (const log of logs) {
    counts[log.level]++;
  }
  return counts;
}

interface LogLevelButtonProps {
  level: LogLevel;
  count: number;
  visible: boolean;
  onToggle: () => void;
  label: string;
}

function LogLevelButton({ level, count, visible, onToggle, label }: LogLevelButtonProps) {
  return (
    <Button
      variant={visible ? "secondary" : "ghost"}
      size="sm"
      className={cn(
        "h-6 text-[11px] px-2 gap-1 transition-all",
        visible ? levelBgColors[level] : "opacity-50 hover:opacity-80"
      )}
      onClick={onToggle}
    >
      <span className={cn("size-1.5 rounded-full", levelDotColors[level])} />
      <span className={cn("font-medium", levelColors[level])}>{label}</span>
      {count > 0 && (
        <span className="text-muted-foreground/70">{count}</span>
      )}
    </Button>
  );
}

interface LogViewerProps {
  logs: LogLine[];
  visibleLevels: Record<LogLevel, boolean>;
  autoScroll: boolean;
  wrapText: boolean;
  searchText: string;
  selectedIds: Set<number>;
  onToggleSelect: (id: number) => void;
  onCopySelected: () => void;
  searchHighlight: string;
}

function LogViewer({
  logs,
  visibleLevels,
  autoScroll,
  wrapText,
  searchText,
  selectedIds,
  onToggleSelect,
  searchHighlight,
}: LogViewerProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  const filteredLogs = useMemo(() => {
    let result = logs.filter((l) => visibleLevels[l.level]);
    if (searchText.trim()) {
      const q = searchText.toLowerCase();
      result = result.filter(
        (l) => l.message.toLowerCase().includes(q) || l.level.includes(q)
      );
    }
    return result;
  }, [logs, visibleLevels, searchText]);

  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [filteredLogs, autoScroll]);

  const renderHighlightedText = (text: string, highlight: string) => {
    if (!highlight.trim()) return text;
    const idx = text.toLowerCase().indexOf(highlight.toLowerCase());
    if (idx === -1) return text;
    return (
      <>
        {text.slice(0, idx)}
        <mark className="bg-yellow-300/50 dark:bg-yellow-500/30 rounded px-0.5">
          {text.slice(idx, idx + highlight.length)}
        </mark>
        {text.slice(idx + highlight.length)}
      </>
    );
  };

  return (
    <div
      ref={scrollRef}
      className={cn(
        "h-full overflow-y-auto rounded-xl bg-muted/50 p-3 font-mono text-xs leading-relaxed",
        wrapText ? "whitespace-pre-wrap break-all" : "whitespace-pre overflow-x-auto"
      )}
    >
      {filteredLogs.length === 0 ? (
        <p className="text-muted-foreground/50 select-none text-center py-4">
          {logs.length === 0 ? "等待启动..." : "无匹配日志"}
        </p>
      ) : (
        filteredLogs.map((log) => (
          <div
            key={log.id}
            onClick={() => onToggleSelect(log.id)}
            className={cn(
              "flex gap-2 py-0.5 px-1 rounded cursor-pointer transition-colors",
              selectedIds.has(log.id)
                ? "bg-primary/10 ring-1 ring-primary/30"
                : "hover:bg-muted/50"
            )}
          >
            <span className="shrink-0 text-muted-foreground/60 select-none">
              [{log.timestamp}]
            </span>
            <span className={cn("shrink-0 select-none", levelColors[log.level])}>
              [{log.level.toUpperCase()}]
            </span>
            <span
              className={cn(
                "break-all flex-1",
                levelColors[log.level]
              )}
            >
              {searchHighlight
                ? renderHighlightedText(log.message, searchHighlight)
                : log.message}
            </span>
          </div>
        ))
      )}
    </div>
  );
}

export function LaunchConsole() {
  const { t } = useI18n();
  const { logs: rawLogs, clearLogs, status, cancelLaunch } = useLaunchContext();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<ConsoleTab>("logs");

  const [visibleLevels, setVisibleLevels] = useState<Record<LogLevel, boolean>>({
    fatal: true,
    error: true,
    warn: true,
    info: true,
    debug: false,
  });
  const [autoScroll, setAutoScroll] = useState(true);
  const [wrapText, setWrapText] = useState(true);
  const [searchText, setSearchText] = useState("");
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

  const logs: LogLine[] = useMemo(() => {
    return rawLogs.map((l) => ({
      id: l.id,
      timestamp: l.timestamp,
      level: autoGuessLevel(l.level === "warn" ? "WARN" : l.level === "error" ? "ERROR" : "INFO"),
      message: l.message,
    }));
  }, [rawLogs]);

  const levelCounts = useMemo(() => countLevels(logs), [logs]);

  const toggleLevel = useCallback((level: LogLevel) => {
    setVisibleLevels((prev) => ({ ...prev, [level]: !prev[level] }));
  }, []);

  const toggleSelect = useCallback((id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const selectedText = useMemo(() => {
    if (selectedIds.size === 0) return "";
    return logs
      .filter((l) => selectedIds.has(l.id))
      .map((l) => l.message)
      .join("\n");
  }, [logs, selectedIds]);

  const handleCopySelected = useCallback(() => {
    if (!selectedText) return;
    navigator.clipboard.writeText(selectedText).catch(() => {});
  }, [selectedText]);

  const handleCopyAll = useCallback(() => {
    const allText = logs.map((l) => `[${l.timestamp}] [${l.level.toUpperCase()}] ${l.message}`).join("\n");
    navigator.clipboard.writeText(allText).catch(() => {});
  }, [logs]);

  const handleExport = useCallback(() => {
    const content = logs
      .map((l) => `[${l.timestamp}] [${l.level.toUpperCase()}] ${l.message}`)
      .join("\n");
    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
    a.download = `minecraft-exported-logs-${timestamp}.log`;
    a.click();
    URL.revokeObjectURL(url);
  }, [logs]);

  const handleClear = useCallback(() => {
    clearLogs();
    setSelectedIds(new Set());
    setSearchText("");
  }, [clearLogs]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "a") {
        e.preventDefault();
        const filtered = logs.filter((l) => visibleLevels[l.level]);
        setSelectedIds(new Set(filtered.map((l) => l.id)));
      }
      if ((e.ctrlKey || e.metaKey) && e.key === "c" && selectedIds.size > 0) {
        e.preventDefault();
        handleCopySelected();
      }
    },
    [logs, visibleLevels, selectedIds, handleCopySelected]
  );

  const isRunning = status === "running" || status === "launching" || status === "preparing";

  const switchTab = (t: ConsoleTab) => setActiveTab(t);

  return (
    <>
      <Card
        size="sm"
        className="flex flex-col min-h-0 flex-1 min-h-[260px] max-h-[70vh]"
      >
        <CardHeader className="flex-row items-center justify-between py-2 px-3">
          <CardTitle className="flex items-center gap-2 text-sm">
            <Terminal className="size-4 text-primary" />
            {t("launch.log.title")}
            {activeTab === "logs" && logs.length > 0 && (
              <span className="text-[10px] text-muted-foreground font-normal">
                ({logs.length})
              </span>
            )}
          </CardTitle>
          <div className="flex items-center gap-1">
            <div className="mr-1 flex rounded-md border border-border bg-muted/30 p-0.5">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => switchTab("logs")}
                className={cn(
                  "h-6 px-2 text-[11px]",
                  activeTab === "logs"
                    ? "bg-background shadow-sm text-primary font-medium"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <Terminal className="size-3 mr-1" />
                启动日志
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => switchTab("analysis")}
                className={cn(
                  "h-6 px-2 text-[11px]",
                  activeTab === "analysis"
                    ? "bg-background shadow-sm text-emerald-600 font-medium"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <Puzzle className="size-3 mr-1" />
                依赖解析
              </Button>
            </div>
            <AnimatePresence>
              {activeTab === "logs" && logs.length > 0 && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.8 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0.8 }}
                  transition={{ duration: 0.15 }}
                  className="flex items-center gap-1"
                >
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={handleCopyAll}
                    title={t("launch.log.copyAll")}
                  >
                    <Copy className="size-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={handleExport}
                    title={t("launch.log.export")}
                  >
                    <Download className="size-3" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 p-0"
                    onClick={handleClear}
                    title={t("common.clear")}
                  >
                    <Trash2 className="size-3" />
                  </Button>
                </motion.div>
              )}
            </AnimatePresence>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 w-6 p-0"
              onClick={() => setOpen(true)}
              title={t("launch.log.expand")}
            >
              <Maximize2 className="size-3.5" />
            </Button>
          </div>
        </CardHeader>

        {activeTab === "logs" && (
          <>
            <div className="shrink-0 border-t border-border px-3 py-2 space-y-2">
              <div className="flex items-center gap-1 flex-wrap">
                {LOG_LEVELS.map((level) => (
                  <LogLevelButton
                    key={level}
                    level={level}
                    count={levelCounts[level]}
                    visible={visibleLevels[level]}
                    onToggle={() => toggleLevel(level)}
                    label={t(`launch.log.level.${level}` as any)}
                  />
                ))}
              </div>

              <div className="flex items-center gap-3 text-xs text-muted-foreground">
                <label className="flex items-center gap-1.5 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    checked={autoScroll}
                    onChange={(e) => setAutoScroll(e.target.checked)}
                    className="size-3 accent-primary"
                  />
                  {t("launch.log.autoScroll")}
                </label>
                <label className="flex items-center gap-1.5 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    checked={wrapText}
                    onChange={(e) => setWrapText(e.target.checked)}
                    className="size-3 accent-primary"
                  />
                  {t("launch.log.wrapText")}
                </label>
                <div className="flex-1 relative">
                  <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-muted-foreground" />
                  <input
                    type="text"
                    value={searchText}
                    onChange={(e) => setSearchText(e.target.value)}
                    placeholder={t("launch.log.search")}
                    className="w-full h-6 pl-7 pr-2 text-xs rounded bg-muted/50 border border-border focus:outline-none focus:ring-1 focus:ring-primary/30"
                  />
                </div>
              </div>

              {selectedIds.size > 0 && (
                <motion.div
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  className="flex items-center gap-2 text-xs text-primary"
                >
                  <span>{t("launch.log.selected", { count: selectedIds.size })}</span>
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-5 text-[10px] px-2"
                    onClick={handleCopySelected}
                  >
                    <Copy className="size-3 mr-1" />
                    {t("launch.log.copySelected")}
                  </Button>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-5 text-[10px] px-2"
                    onClick={() => setSelectedIds(new Set())}
                  >
                    {t("launch.log.deselectAll")}
                  </Button>
                </motion.div>
              )}
            </div>

            <CardContent className="flex-1 min-h-0 p-0">
              <div
                ref={scrollRef}
                className="h-full px-3 pb-3"
                onKeyDown={handleKeyDown}
                tabIndex={0}
              >
                <LogViewer
                  logs={logs}
                  visibleLevels={visibleLevels}
                  autoScroll={autoScroll}
                  wrapText={wrapText}
                  searchText={searchText}
                  selectedIds={selectedIds}
                  onToggleSelect={toggleSelect}
                  onCopySelected={handleCopySelected}
                  searchHighlight={searchText}
                />
              </div>
            </CardContent>

            <div className="shrink-0 border-t border-border px-3 py-2 flex items-center justify-between">
              <div className="flex items-center gap-3 text-[11px] text-muted-foreground">
                <span className="flex items-center gap-1">
                  <span className="size-1.5 rounded-full bg-red-500" />
                  {levelCounts.error} {t("launch.log.level.error")}
                </span>
                <span className="flex items-center gap-1">
                  <span className="size-1.5 rounded-full bg-amber-500" />
                  {levelCounts.warn} {t("launch.log.level.warn")}
                </span>
                <span className="flex items-center gap-1">
                  <span className="size-1.5 rounded-full bg-sky-500" />
                  {levelCounts.debug} {t("launch.log.level.debug")}
                </span>
              </div>
              <div className="flex items-center gap-1">
                {isRunning && (
                  <Button
                    variant="outline"
                    size="sm"
                    className="h-6 text-[11px] text-destructive border-destructive/50 hover:bg-destructive/10"
                    onClick={cancelLaunch}
                  >
                    <Square className="size-3 mr-1" />
                    {t("launch.log.terminate")}
                  </Button>
                )}
              </div>
            </div>
          </>
        )}

        {activeTab === "analysis" && (
          <CardContent className="flex-1 min-h-0 overflow-y-auto px-3 py-2">
            <ModDependencyPanel />
          </CardContent>
        )}
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="flex flex-col h-[85vh] max-w-4xl">
          <DialogHeader className="flex-row items-center justify-between">
            <DialogTitle className="flex items-center gap-2 text-sm">
              <Terminal className="size-4 text-primary" />
              {t("launch.log.title")}
              {activeTab === "logs" && logs.length > 0 && (
                <span className="text-[10px] text-muted-foreground font-normal">
                  ({logs.length})
                </span>
              )}
            </DialogTitle>
            <div className="flex items-center gap-1">
              <div className="mr-1 flex rounded-md border border-border bg-muted/30 p-0.5">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => switchTab("logs")}
                  className={cn(
                    "h-7 px-2 text-xs",
                    activeTab === "logs"
                      ? "bg-background shadow-sm text-primary font-medium"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  <Terminal className="size-3 mr-1" />
                  启动日志
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => switchTab("analysis")}
                  className={cn(
                    "h-7 px-2 text-xs",
                    activeTab === "analysis"
                      ? "bg-background shadow-sm text-emerald-600 font-medium"
                      : "text-muted-foreground hover:text-foreground"
                  )}
                >
                  <Puzzle className="size-3 mr-1" />
                  依赖解析
                </Button>
              </div>
              {activeTab === "logs" && logs.length > 0 && (
                <>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={handleCopyAll}
                  >
                    <Copy className="size-3 mr-1" />
                    {t("launch.log.copyAll")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={handleExport}
                  >
                    <Download className="size-3 mr-1" />
                    {t("launch.log.export")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={handleClear}
                  >
                    <Trash2 className="size-3 mr-1" />
                    {t("common.clear")}
                  </Button>
                </>
              )}
            </div>
          </DialogHeader>

          {activeTab === "logs" && (
            <>
              <div className="shrink-0 border-t border-border px-4 py-2 space-y-2">
                <div className="flex items-center gap-1 flex-wrap">
                  {LOG_LEVELS.map((level) => (
                    <LogLevelButton
                      key={level}
                      level={level}
                      count={levelCounts[level]}
                      visible={visibleLevels[level]}
                      onToggle={() => toggleLevel(level)}
                      label={t(`launch.log.level.${level}` as any)}
                    />
                  ))}
                </div>
                <div className="flex items-center gap-3 text-xs text-muted-foreground">
                  <label className="flex items-center gap-1.5 cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={autoScroll}
                      onChange={(e) => setAutoScroll(e.target.checked)}
                      className="size-3 accent-primary"
                    />
                    {t("launch.log.autoScroll")}
                  </label>
                  <label className="flex items-center gap-1.5 cursor-pointer select-none">
                    <input
                      type="checkbox"
                      checked={wrapText}
                      onChange={(e) => setWrapText(e.target.checked)}
                      className="size-3 accent-primary"
                    />
                    {t("launch.log.wrapText")}
                  </label>
                  <div className="flex-1 relative">
                    <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-muted-foreground" />
                    <input
                      type="text"
                      value={searchText}
                      onChange={(e) => setSearchText(e.target.value)}
                      placeholder={t("launch.log.search")}
                      className="w-full h-6 pl-7 pr-2 text-xs rounded bg-muted/50 border border-border focus:outline-none focus:ring-1 focus:ring-primary/30"
                    />
                  </div>
                </div>
                {selectedIds.size > 0 && (
                  <motion.div
                    initial={{ opacity: 0, height: 0 }}
                    animate={{ opacity: 1, height: "auto" }}
                    className="flex items-center gap-2 text-xs text-primary"
                  >
                    <span>{t("launch.log.selected", { count: selectedIds.size })}</span>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-5 text-[10px] px-2"
                      onClick={handleCopySelected}
                    >
                      <Copy className="size-3 mr-1" />
                      {t("launch.log.copySelected")}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-5 text-[10px] px-2"
                      onClick={() => setSelectedIds(new Set())}
                    >
                      {t("launch.log.deselectAll")}
                    </Button>
                  </motion.div>
                )}
              </div>

              <div className="flex-1 min-h-0 p-4 pt-0 overflow-y-auto">
                <LogViewer
                  logs={logs}
                  visibleLevels={visibleLevels}
                  autoScroll={autoScroll}
                  wrapText={wrapText}
                  searchText={searchText}
                  selectedIds={selectedIds}
                  onToggleSelect={toggleSelect}
                  onCopySelected={handleCopySelected}
                  searchHighlight={searchText}
                />
              </div>
            </>
          )}

          {activeTab === "analysis" && (
            <div className="flex-1 min-h-0 overflow-y-auto p-4 pt-2 border-t border-border">
              <ModDependencyPanel />
            </div>
          )}
        </DialogContent>
      </Dialog>
    </>
  );
}