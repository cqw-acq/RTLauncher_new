"use client";

import { AnimatePresence, motion } from "framer-motion";
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
  Clock,
  GripVertical,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { fadeSlideUp } from "@/lib/motion";

function TaskRow({ task, onRemove, onCancel }: { task: DownloadTask; onRemove: () => void; onCancel: () => void }) {
  return (
    <motion.div
      layout
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.2 }}
      className="overflow-hidden"
    >
      <div className="flex flex-col gap-1.5 px-3 py-2 border-b border-border last:border-b-0">
        <div className="flex items-center gap-2 min-w-0">
          {/* 状态图标 */}
          {task.status === "queued" && (
            <Clock className="size-3.5 text-muted-foreground shrink-0" />
          )}
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
          {task.status === "queued" ? (
            <Button
              variant="ghost"
              size="icon-sm"
              className="size-5 shrink-0"
              onClick={onCancel}
              title="取消排队"
              aria-label="取消排队"
            >
              <X className="size-3" />
            </Button>
          ) : task.status === "downloading" ? (
            <div className="flex items-center gap-1 shrink-0">
              <span className="text-[11px] text-muted-foreground tabular-nums">
                {task.progress !== undefined ? `${task.progress.toFixed(1)}%` : "校验中..."}
              </span>
              <Button
                variant="ghost"
                size="icon-sm"
                className="size-5"
                onClick={onCancel}
                title="取消下载"
                aria-label="取消下载"
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
              title="移除任务"
              aria-label="移除任务"
            >
              <X className="size-3" />
            </Button>
          )}
        </div>

        {/* 进度条 */}
        {task.status === "downloading" && (
          <Progress value={task.progress} className="h-1" />
        )}

        {/* 排队等待 */}
        {task.status === "queued" && (
          <p className="text-[11px] text-muted-foreground truncate">排队等待中...</p>
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
    </motion.div>
  );
}

export function DownloadTaskList() {
  const { tasks, clearFinished, removeTask, cancelDownload } = useDownloadManager();
  const [collapsed, setCollapsed] = useState(false);

  const activeCount = tasks.filter((t) => t.status === "downloading").length;
  const queuedCount = tasks.filter((t) => t.status === "queued").length;
  const finishedCount = tasks.length - activeCount - queuedCount;

  // 拖拽状态
  const panelRef = useRef<HTMLDivElement | null>(null);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const draggingRef = useRef(false);
  const dragStartRef = useRef<{ offsetX: number; offsetY: number }>({ offsetX: 0, offsetY: 0 });

  const clampToViewport = useCallback((next: { x: number; y: number }) => {
    const panel = panelRef.current;
    const width = panel?.offsetWidth ?? 0;
    const height = panel?.offsetHeight ?? 0;
    return {
      x: Math.max(0, Math.min(Math.max(0, window.innerWidth - width), next.x)),
      y: Math.max(0, Math.min(Math.max(0, window.innerHeight - height), next.y)),
    };
  }, []);

  const onMouseDown = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    // 只响应鼠标左键
    if (e.button !== 0) return;
    const panel = panelRef.current;
    if (!panel) return;
    const rect = panel.getBoundingClientRect();
    draggingRef.current = true;
    dragStartRef.current = {
      offsetX: e.clientX - rect.left,
      offsetY: e.clientY - rect.top,
    };
    // 防止选中文字等副作用
    e.preventDefault();
  }, []);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!draggingRef.current) return;
      const { offsetX, offsetY } = dragStartRef.current;
      setPosition(clampToViewport({
        x: e.clientX - offsetX,
        y: e.clientY - offsetY,
      }));
    };
    const onUp = () => {
      draggingRef.current = false;
    };
    const onResize = () => {
      setPosition((current) => current ? clampToViewport(current) : current);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("resize", onResize);
    };
  }, [clampToViewport]);

  const panelStyle: React.CSSProperties = position
    ? { left: position.x, top: position.y, right: "auto", bottom: "auto" }
    : {};

  return (
    <AnimatePresence>
      {tasks.length > 0 && (
        <motion.div
          ref={panelRef}
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          exit="exit"
          style={panelStyle}
          className="fixed bottom-4 right-4 z-50 w-80 rounded-xl border border-border bg-card shadow-lg overflow-hidden select-none"
        >
          {/* 头部 */}
          <div
            onMouseDown={onMouseDown}
            className="flex items-center justify-between px-3 py-2 bg-muted/50 border-b border-border cursor-grab active:cursor-grabbing"
          >
            <span className="text-xs font-medium flex items-center gap-1">
              <GripVertical className="size-3 text-muted-foreground" />
              下载任务
              {activeCount > 0 && (
                <span className="text-muted-foreground ml-1">
                  ({activeCount} 进行中{queuedCount > 0 ? `，${queuedCount} 排队` : ""})
                </span>
              )}
              {activeCount === 0 && queuedCount > 0 && (
                <span className="text-muted-foreground ml-1">
                  ({queuedCount} 排队中)
                </span>
              )}
            </span>
            <div className="flex items-center gap-0.5">
              <AnimatePresence>
                {finishedCount > 0 && (
                  <motion.div
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.8 }}
                    transition={{ duration: 0.15 }}
                  >
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      className="size-6"
                      onClick={clearFinished}
                      title="清除已完成"
                      aria-label="清除已完成任务"
                      onMouseDown={(e) => e.stopPropagation()}
                    >
                      <Trash2 className="size-3" />
                    </Button>
                  </motion.div>
                )}
              </AnimatePresence>
              <Button
                variant="ghost"
                size="icon-sm"
                className="size-6"
                onClick={() => setCollapsed((prev) => !prev)}
                onMouseDown={(e) => e.stopPropagation()}
                title={collapsed ? "展开下载任务" : "折叠下载任务"}
                aria-label={collapsed ? "展开下载任务" : "折叠下载任务"}
              >
                {collapsed ? (
                  <ChevronUp className="size-3" />
                ) : (
                  <ChevronDown className="size-3" />
                )}
              </Button>
            </div>
          </div>

          {/* 任务列表（折叠/展开动画） */}
          <AnimatePresence initial={false}>
            {!collapsed && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.2, ease: "easeInOut" }}
                className="overflow-hidden"
              >
                <div className="max-h-60 overflow-y-auto">
                  <AnimatePresence initial={false}>
                    {tasks.map((task) => (
                      <TaskRow
                        key={task.taskId}
                        task={task}
                        onRemove={() => removeTask(task.taskId)}
                        onCancel={() => cancelDownload(task.taskId)}
                      />
                    ))}
                  </AnimatePresence>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </motion.div>
      )}
    </AnimatePresence>
  );
}