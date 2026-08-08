"use client";

import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogClose,
} from "@/components/ui/dialog";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n";
import {
  AlertTriangle,
  BarChart3,
  CheckCircle2,
  Code,
  Copy,
  Download,
  FileBarChart,
  Flag,
  List,
  Loader2,
  Play,
  XCircle,
  X,
} from "lucide-react";
import { formatDuration } from "@/components/launch/launch-analyzer";
import type { LaunchAnalysisReport, Log4jLogEntry } from "@/types";

type ReportFinalStatus = LaunchAnalysisReport["finalStatus"];
const FINAL_STATUS_KEYS: Record<ReportFinalStatus, string> = {
  running: "launch.report.statusRunning",
  stopped: "launch.report.statusStopped",
  error: "launch.report.statusError",
  in_progress: "launch.report.statusInProgress",
  idle: "launch.report.statusIdle",
  timeout: "launch.report.statusTimeout",
} as const;

function copyText(text: string) {
  try {
    void navigator.clipboard?.writeText(text);
  } catch {
    /* ignore */
  }
}

function reportToPlainText(report: LaunchAnalysisReport): string {
  const lines: string[] = [];
  lines.push("=== Minecraft Launch Report ===");
  lines.push(`Status: ${report.finalStatus}`);
  if (report.exitCode != null) lines.push(`Exit code: ${report.exitCode}`);
  if (report.startedAt) lines.push(`Started: ${new Date(report.startedAt).toLocaleString()}`);
  if (report.endedAt) lines.push(`Ended:   ${new Date(report.endedAt).toLocaleString()}`);
  lines.push(`Total duration: ${formatDuration(report.totalDurationMs)}`);
  lines.push("");
  lines.push("--- Stage Breakdown ---");
  for (const s of report.stages) {
    const mark = s.completed ? "[OK]" : s.enteredAt ? "--" : "  ";
    lines.push(`${mark} ${s.name.padEnd(18)} ${formatDuration(s.durationMs).padStart(10)}  (${s.logCount} logs)`);
  }
  lines.push("");
  lines.push("--- Overview ---");
  if (report.detectedMcVersion) lines.push(`MC version: ${report.detectedMcVersion}`);
  if (report.detectedLoader) lines.push(`Loader:     ${report.detectedLoader}`);
  if (report.detectedModCount != null) lines.push(`Mod count:  ${report.detectedModCount}`);
  lines.push(`Warnings:  ${report.warnCount}`);
  lines.push(`Errors:    ${report.errorCount}`);
  lines.push(`Log lines: ${report.totalLogLines}`);
  if (report.errorSamples.length) {
    lines.push("");
    lines.push("--- Error samples ---");
    for (const e of report.errorSamples) lines.push(`- ${e}`);
  }
  if (report.failureHints.length) {
    lines.push("");
    lines.push("--- Hints ---");
    for (const h of report.failureHints) lines.push(`- ${h}`);
  }
  if (report.log4jLogs && report.log4jLogs.length > 0) {
    lines.push("");
    lines.push("--- Log4j Logs ---");
    for (const l of report.log4jLogs) {
      lines.push(`[${l.level}] ${l.timestamp} ${l.logger ?? ""} ${l.message}${l.relatedProblem ? " | " + l.relatedProblem : ""}`);
    }
  }
  if (report.launchParameters) {
    lines.push("");
    lines.push("--- Launch Parameters ---");
    lines.push(report.launchParameters);
  }
  return lines.join("\n");
}

const FAILURE_STATUSES: Set<LaunchAnalysisReport["finalStatus"]> = new Set([
  "error",
  "timeout",
]);

export function LaunchReportButton() {
  const { t } = useI18n();
  const { generateReport, logs, exportLaunchReport } = useLaunchContext();
  const [open, setOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const report = useMemo<LaunchAnalysisReport>(
    () => generateReport(),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [logs, generateReport],
  );

  const hasSomethingToReport = logs.length > 0;
  const failed = FAILURE_STATUSES.has(report.finalStatus) || report.errorCount > 0;

  const handleExportZip = async () => {
    setExporting(true);
    setExportPath(null);
    try {
      const path = await exportLaunchReport();
      setExportPath(path);
    } catch (err) {
      console.error("导出启动报告失败:", err);
    } finally {
      setExporting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          size="sm"
          variant={failed ? "destructive" : "outline"}
          className="w-full gap-2 text-xs"
          disabled={!hasSomethingToReport}
        >
          {report.finalStatus === "in_progress" ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : failed ? (
            <AlertTriangle className="size-3.5" />
          ) : (
            <FileBarChart className="size-3.5" />
          )}
          {t("launch.report.launchAnalysis")}
          {report.errorCount > 0 && (
            <Badge variant="destructive" className="ml-auto text-[10px] px-1 py-0 h-4">
              {report.errorCount}
            </Badge>
          )}
          {report.warnCount > 0 && report.errorCount === 0 && (
            <Badge variant="secondary" className="ml-auto text-[10px] px-1 py-0 h-4">
              {report.warnCount}
            </Badge>
          )}
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl max-h-[85vh] flex flex-col p-0 overflow-hidden gap-0">
        <DialogHeader className="px-6 pt-6 pb-4 shrink-0">
          <div className="flex items-center justify-between gap-3">
            <DialogTitle className="flex items-center gap-2 text-base">
              <FileBarChart className="size-4 text-primary" />
              {t("launch.report.launchAnalysisReport")}
            </DialogTitle>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="ghost"
                className="h-8 px-2 text-xs"
                onClick={() => copyText(reportToPlainText(report))}
              >
                <Copy className="size-3.5 mr-1" />
                {t("launch.report.copyText")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="h-8 px-2 text-xs"
                onClick={() => {
                  const blob = new Blob([reportToPlainText(report)], {
                    type: "text/plain;charset=utf-8",
                  });
                  const url = URL.createObjectURL(blob);
                  const a = document.createElement("a");
                  a.href = url;
                  a.download = `launch-report-${report.startedAt ?? Date.now()}.txt`;
                  a.click();
                  URL.revokeObjectURL(url);
                }}
              >
                <Download className="size-3.5 mr-1" />
                {t("launch.report.exportTxt")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="h-8 px-2 text-xs"
                onClick={handleExportZip}
                disabled={exporting}
              >
                {exporting ? (
                  <Loader2 className="size-3.5 mr-1 animate-spin" />
                ) : (
                  <Download className="size-3.5 mr-1" />
                )}
                {exporting ? t("launch.report.exporting") : t("launch.report.exportZip")}
              </Button>
              <DialogClose asChild>
                <Button size="sm" variant="ghost" className="h-8 w-8 p-0">
                  <X className="size-4" />
                </Button>
              </DialogClose>
            </div>
          </div>
        </DialogHeader>
        <div className="px-6 pb-6 overflow-y-auto space-y-4 flex-1 min-h-0">
          {/* Summary cards */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs flex items-center gap-1.5">
                  <Flag className="size-3.5 text-muted-foreground" />
                  {t("launch.report.finalStatus")}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex items-center gap-2">
                  {report.finalStatus === "error" || report.finalStatus === "timeout" ? (
                    <XCircle className="size-4 text-destructive" />
                  ) : report.finalStatus === "running" || report.finalStatus === "stopped" ? (
                    <CheckCircle2 className="size-4 text-emerald-500" />
                  ) : report.finalStatus === "in_progress" ? (
                    <Play className="size-4 text-primary" />
                  ) : (
                    <Loader2 className="size-4 text-muted-foreground" />
                  )}
                  <span className="text-sm font-semibold">
                    {t(FINAL_STATUS_KEYS[report.finalStatus] as any)}
                  </span>
                </div>
              </CardContent>
            </Card>
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs flex items-center gap-1.5">
                  <BarChart3 className="size-3.5 text-muted-foreground" />
                  {t("launch.report.totalDuration")}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <span className="text-sm font-mono font-semibold tabular-nums">
                  {formatDuration(report.totalDurationMs)}
                </span>
              </CardContent>
            </Card>
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs">{t("launch.report.exitCode")}</CardTitle>
              </CardHeader>
              <CardContent>
                <span className="text-sm font-mono font-semibold tabular-nums">
                  {report.exitCode == null ? "—" : report.exitCode}
                </span>
              </CardContent>
            </Card>
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-xs">{t("launch.report.logLines")}</CardTitle>
              </CardHeader>
              <CardContent>
                <span className="text-sm font-mono font-semibold tabular-nums">
                  {report.totalLogLines.toLocaleString()}
                </span>
              </CardContent>
            </Card>
          </div>

          {/* Detection overview - REMOVED */}

          {/* Stage breakdown - REMOVED */}

          {/* Error samples */}
          {report.errorSamples.length > 0 && (
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-sm flex items-center gap-2 text-destructive">
                  <XCircle className="size-4" />
                  {t("launch.report.errorSamples")}
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-1.5">
                {report.errorSamples.map((e, i) => (
                  <div
                    key={i}
                    className="rounded-lg bg-destructive/5 border border-destructive/10 p-2 text-[11px] text-destructive break-words font-mono"
                  >
                    {e}
                  </div>
                ))}
              </CardContent>
            </Card>
          )}

          {/* Export success notification */}
          {exportPath && (
            <Card size="sm" className="border-emerald-500/30 bg-emerald-500/5">
              <CardContent className="py-3 px-4 flex items-center gap-3">
                <CheckCircle2 className="size-4 text-emerald-500 shrink-0" />
                <div className="flex-1 min-w-0">
                  <p className="text-xs font-medium text-emerald-700 dark:text-emerald-400">
                    {t("launch.report.exportSuccess")}
                  </p>
                  <p className="text-[11px] text-emerald-600/80 dark:text-emerald-500/70 truncate font-mono">
                    {exportPath}
                  </p>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Failure hints */}
          {report.failureHints.length > 0 && (
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-sm flex items-center gap-2 text-amber-600">
                  <AlertTriangle className="size-4" />
                  {t("launch.report.suggestedFixes")}
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-1.5">
                {report.failureHints.map((hint, i) => (
                  <div
                    key={i}
                    className="rounded-lg bg-amber-500/10 border border-amber-500/15 p-2.5 text-[11px] text-amber-700 dark:text-amber-400">
                    • {hint}
                  </div>
                ))}
              </CardContent>
            </Card>
          )}

          {/* Log4j logs with problem association */}
          {report.log4jLogs && report.log4jLogs.length > 0 && (
            <Log4jLogsSection logs={report.log4jLogs} t={t} />
          )}

          {/* Launch parameters */}
          {report.launchParameters && (
            <Card size="sm">
              <CardHeader className="pb-2">
                <CardTitle className="text-sm flex items-center gap-2">
                  <Code className="size-4 text-primary" />
                  {t("launch.report.launchParameters")}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <div className="max-h-48 overflow-y-auto rounded-lg bg-muted/50 p-3 text-[11px] font-mono text-muted-foreground whitespace-pre-wrap break-all">
                  {report.launchParameters}
                </div>
              </CardContent>
            </Card>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function Log4jLogsSection({ logs, t }: { logs: Log4jLogEntry[]; t: (key: TranslationKey, values?: Record<string, string | number>) => string }) {
  const levelColor = (level: string) => {
    const l = level.toUpperCase();
    if (l === "ERROR" || l === "FATAL") return "text-destructive";
    if (l === "WARN" || l === "WARNING") return "text-amber-500";
    if (l === "INFO") return "text-emerald-500";
    return "text-muted-foreground";
  };

  const levelBg = (level: string) => {
    const l = level.toUpperCase();
    if (l === "ERROR" || l === "FATAL") return "bg-destructive/5 border-destructive/10";
    if (l === "WARN" || l === "WARNING") return "bg-amber-500/5 border-amber-500/10";
    return "bg-muted/30 border-muted/10";
  };

  const problems = logs.filter((l) => l.relatedProblem);
  const others = logs.filter((l) => !l.relatedProblem);

  return (
    <Card size="sm">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm flex items-center gap-2">
          <List className="size-4 text-primary" />
          {t("launch.report.log4jLogs")}
          <span className="text-xs text-muted-foreground font-normal">
            ({logs.length})
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 max-h-72 overflow-y-auto">
        {problems.length > 0 && (
          <div className="space-y-1.5">
            <p className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              {t("launch.report.relatedProblems")}
            </p>
            {problems.map((log, i) => (
              <div
                key={`prob-${i}`}
                className={`rounded-lg border p-2 text-[11px] font-mono break-words ${levelBg(log.level)}`}
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className={`font-bold ${levelColor(log.level)}`}>
                    [{log.level}]
                  </span>
                  <span className="text-muted-foreground">{log.timestamp}</span>
                  {log.logger && (
                    <span className="text-muted-foreground truncate max-w-[200px]">
                      {log.logger}
                    </span>
                  )}
                </div>
                <div className="text-foreground/90">{log.message}</div>
                {log.relatedProblem && (
                  <div className="mt-1 pt-1 border-t border-border/50 text-amber-600 dark:text-amber-400 text-[10px]">
                    {log.relatedProblem}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
        {others.length > 0 && (
          <div className="space-y-1.5">
            {problems.length > 0 && (
              <p className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                {t("launch.report.otherLogs")}
              </p>
            )}
            {others.slice(0, 20).map((log, i) => (
              <div
                key={`other-${i}`}
                className={`rounded-lg border p-2 text-[11px] font-mono break-words ${levelBg(log.level)}`}
              >
                <div className="flex items-center gap-2 mb-1">
                  <span className={`font-bold ${levelColor(log.level)}`}>
                    [{log.level}]
                  </span>
                  <span className="text-muted-foreground">{log.timestamp}</span>
                  {log.logger && (
                    <span className="text-muted-foreground truncate max-w-[200px]">
                      {log.logger}
                    </span>
                  )}
                </div>
                <div className="text-foreground/90">{log.message}</div>
              </div>
            ))}
            {others.length > 20 && (
              <p className="text-[10px] text-muted-foreground text-center">
                {t("launch.report.moreLogs", { count: others.length - 20 })}
              </p>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}