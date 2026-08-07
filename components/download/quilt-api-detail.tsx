"use client";

import { useState, useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { LoaderVersionList } from "@/components/download/loader-version-list";
import { ArrowLeft, Loader2, Download } from "lucide-react";
import { slideInFromRight, fadeIn } from "@/lib/motion";
import type { LoaderVersion } from "@/types";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadManager } from "@/components/download/download-provider";

interface QuiltApiDetailProps {
  mcVersion: string;
  onBack: () => void;
}

const QUILT_LOADER_DEFAULT = "0.26.0";

export function QuiltApiDetail({ mcVersion, onBack }: QuiltApiDetailProps) {
  const [apiVersions, setApiVersions] = useState<LoaderVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [showNameDialog, setShowNameDialog] = useState(false);
  const [pendingApi, setPendingApi] = useState<LoaderVersion | null>(null);
  const [instanceNameInput, setInstanceNameInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const { startQuiltDownload } = useDownloadManager();

  useEffect(() => {
    const fetchApiVersions = async () => {
      setLoading(true);
      try {
        const result = await invoke<{ id: string; version: string }[]>(
          "get_quilt_api_versions",
          { mcVersion }
        );
        const versions: LoaderVersion[] = result.map(v => ({
          id: v.id,
          version: v.version,
          releaseDate: "",
          isRecommended: false
        }));
        setApiVersions(versions);
      } catch (err) {
        console.error("获取Quilt API版本列表失败:", err);
        setApiVersions([]);
      } finally {
        setLoading(false);
      }
    };
    fetchApiVersions();
  }, [mcVersion]);

  const defaultName = (apiVer: string) => `${mcVersion}-Quilt-${QUILT_LOADER_DEFAULT}-API-${apiVer}`;

  const confirmDownload = async () => {
    if (!pendingApi) return;
    const name = instanceNameInput.trim();
    const instanceName = name.length > 0 ? name : defaultName(pendingApi.version);

    setShowNameDialog(false);
    const api = pendingApi;
    setPendingApi(null);

    try {
      const taskId = await startQuiltDownload(mcVersion, QUILT_LOADER_DEFAULT, api.version, instanceName);
      console.log(`Quilt API 下载任务已启动，任务ID: ${taskId}`);
    } catch (err) {
      console.error("下载并安装Quilt API失败:", err);
    }
  };

  const handleInstall = async (apiVersion: LoaderVersion) => {
    setPendingApi(apiVersion);
    setInstanceNameInput(defaultName(apiVersion.version));
    setShowNameDialog(true);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 50);
  };

  return (
    <>
    <div className="flex h-full flex-col gap-4">
      {/* 返回按钮 + 版本信息头 */}
      <div className="flex items-center gap-3 shrink-0">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onBack}
          aria-label="返回"
        >
          <ArrowLeft className="size-4" />
        </Button>
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold leading-none">
            Quilt API
          </h2>
          <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
            {mcVersion}
          </Badge>
        </div>
      </div>

      {/* 子标题区域 */}
      <div className="shrink-0">
        <h3 className="text-sm font-medium text-muted-foreground">
          选择 Quilt API 版本
        </h3>
        <p className="text-xs text-muted-foreground/70 mt-0.5">
          选择一个 Quilt API 版本进行安装
        </p>
      </div>

      {/* 内容区域 */}
      <div className="flex-1 min-h-0 overflow-y-auto p-1">
        <AnimatePresence mode="wait">
          {loading ? (
            <motion.div
              key="loading"
              className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
              variants={fadeIn}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <Loader2 className="size-8 animate-spin" />
              <p className="text-sm">正在获取版本列表...</p>
            </motion.div>
          ) : (
            <motion.div
              key="list"
              variants={fadeIn}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <LoaderVersionList
                loaderName="Quilt API"
                versions={apiVersions}
                onInstall={handleInstall}
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>

    <Dialog open={showNameDialog} onOpenChange={setShowNameDialog}>
      <DialogContent className="!max-w-lg p-0">
        <DialogHeader>
          <DialogTitle>实例名称</DialogTitle>
        </DialogHeader>
        <div className="p-5 space-y-4">
          <div className="space-y-2">
            <p className="text-sm text-muted-foreground">
              请为这个实例命名，未填写则使用默认名称：
              <code className="mx-1 px-1.5 py-0.5 rounded bg-muted text-xs">
                {pendingApi ? defaultName(pendingApi.version) : ""}
              </code>
            </p>
            <Input
              ref={inputRef}
              placeholder={pendingApi ? defaultName(pendingApi.version) : ""}
              value={instanceNameInput}
              onChange={(e) => setInstanceNameInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") confirmDownload();
              }}
            />
          </div>
          <div className="flex items-center justify-end gap-2">
            <Button
              variant="outline"
              onClick={() => {
                setShowNameDialog(false);
                setPendingApi(null);
              }}
            >
              取消
            </Button>
            <Button onClick={confirmDownload}>开始下载</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
    </>
  );
}
