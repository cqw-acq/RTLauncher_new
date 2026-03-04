"use client";

import { useEffect, useRef } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { Terminal, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";

const levelColors: Record<string, string> = {
  info: "text-muted-foreground",
  warn: "text-amber-500",
  error: "text-destructive",
};

/**
 * 启动日志控制台
 * 显示启动过程的日志输出
 */
export function LaunchConsole() {
  const { logs, clearLogs } = useLaunchContext();
  const scrollRef = useRef<HTMLDivElement>(null);

  // 自动滚动到底部
  useEffect(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [logs]);

  return (
    <Card size="sm" className="flex flex-col min-h-0">
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Terminal className="size-4 text-primary" />
          启动日志
        </CardTitle>
        {logs.length > 0 && (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            onClick={clearLogs}
          >
            <Trash2 className="size-3 mr-1" />
            清空
          </Button>
        )}
      </CardHeader>
      <CardContent className="flex-1 min-h-0">
        <div
          ref={scrollRef}
          className="h-48 overflow-y-auto rounded-xl bg-muted/50 p-3 font-mono text-xs leading-relaxed"
        >
          {logs.length === 0 ? (
            <p className="text-muted-foreground/50 select-none">
              等待启动...
            </p>
          ) : (
            logs.map((log) => (
              <div key={log.id} className="flex gap-2">
                <span className="shrink-0 text-muted-foreground/60">
                  [{log.timestamp}]
                </span>
                <span
                  className={cn(
                    "break-all",
                    levelColors[log.level] ?? "text-foreground"
                  )}
                >
                  {log.message}
                </span>
              </div>
            ))
          )}
        </div>
      </CardContent>
    </Card>
  );
}
