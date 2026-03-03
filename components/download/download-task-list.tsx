"use client";

import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import {
  useDownloadManager,
  type DownloadTask,
} from "@/components/download/download-provider";
import {
  Loader2,
  CheckCircle2,
  AlertCircle,
  XCircle,
  Ban,
  X,
  Trash2,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import { useState } from "react";

function TaskRow({ task, onRemove, onCancel }: { task: DownloadTask; onRemove: () => void; onCancel: () => void }) {
  return (
    <div className="flex flex-col gap-1.5 px-3 py-2 border-b border-border last:border-b-0">
      <div className="flex items-center gap-2 min-w-0">
        {/* 状态图标 */}
        {task.status === "downloading" && (
          <Loader2 className="size-3.5 animate-spin text-primary shrink-0" />
        )}
        {task.status === "success" && (
          <CheckCircle2 className="size-3.5 text-green-500 shrink-0" />
        )}
        {task.status === "warning" && (
          <AlertCircle className="size-3.5 text-amber-500 shrink-0" />
        )}
        {task.status === "error" && (
          <XCircle className="size-3.5 text-destructive shrink-0" />
        )}
        {task.status === "cancelled" && (
          <Ban className="size-3.5 text-muted-foreground shrink-0" />
        )}

        {/* 标签 */}
        <span className="flex-1 text-xs truncate">{task.label}</span>

        {/* 百分比+取消 / 删除 */}
        {task.status === "downloading" ? (
          <div className="flex items-center gap-1 shrink-0">
            <span className="text-[11px] text-muted-foreground tabular-nums">
              {task.progress.toFixed(1)}%
            </span>
            <Button
              variant="ghost"
              size="icon-sm"
              className="size-5"
              onClick={onCancel}
              title="取消下载"
            >
              <X className="size-3" />
            </Button>
          </div>
        ) : (
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-5 shrink-0"
            onClick={onRemove}
          >
            <X className="size-3" />
          </Button>
        )}
      </div>

      {/* 进度条 */}
      {task.status === "downloading" && (
        <Progress value={task.progress} className="h-1" />
      )}

      {/* 已取消 */}
      {task.status === "cancelled" && (
        <p className="text-[11px] text-muted-foreground truncate">已取消，文件已清理</p>
      )}

      {/* 部分成功警告 */}
      {task.status === "warning" && task.failedCount && (
        <p className="text-[11px] text-amber-500 truncate">
          {task.failedCount} 个文件下载失败，可能影响运行
        </p>
      )}

      {/* 错误信息 */}
      {task.status === "error" && task.error && (
        <p className="text-[11px] text-destructive truncate">{task.error}</p>
      )}
    </div>
  );
}

export function DownloadTaskList() {
  const { tasks, clearFinished, removeTask, cancelDownload } = useDownloadManager();
  const [collapsed, setCollapsed] = useState(false);

  if (tasks.length === 0) return null;

  const activeCount = tasks.filter((t) => t.status === "downloading").length;
  const finishedCount = tasks.length - activeCount;

  return (
    <div className="fixed bottom-4 right-4 z-50 w-80 rounded-xl border border-border bg-card shadow-lg overflow-hidden animate-in fade-in slide-in-from-bottom-4 duration-300">
      {/* 头部 */}
      <div className="flex items-center justify-between px-3 py-2 bg-muted/50 border-b border-border">
        <span className="text-xs font-medium">
          下载任务
          {activeCount > 0 && (
            <span className="text-muted-foreground ml-1">
              ({activeCount} 进行中)
            </span>
          )}
        </span>
        <div className="flex items-center gap-0.5">
          {finishedCount > 0 && (
            <Button
              variant="ghost"
              size="icon-sm"
              className="size-6"
              onClick={clearFinished}
              title="清除已完成"
            >
              <Trash2 className="size-3" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-6"
            onClick={() => setCollapsed((prev) => !prev)}
          >
            {collapsed ? (
              <ChevronUp className="size-3" />
            ) : (
              <ChevronDown className="size-3" />
            )}
          </Button>
        </div>
      </div>

      {/* 任务列表 */}
      {!collapsed && (
        <div className="max-h-60 overflow-y-auto">
          {tasks.map((task) => (
            <TaskRow
              key={task.taskId}
              task={task}
              onRemove={() => removeTask(task.taskId)}
              onCancel={() => cancelDownload(task.taskId)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
