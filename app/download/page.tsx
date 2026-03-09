"use client";

import { useState, useMemo, useEffect } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Download, Loader2, AlertCircle, RefreshCw, Coffee } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  VersionFilterBar,
  type VersionFilter,
} from "@/components/download/version-filter-bar";
import { VersionList } from "@/components/download/version-list";
import { VersionDetail } from "@/components/download/version-detail";
import { useMinecraftVersions } from "@/hooks/use-minecraft-versions";
import { slideInFromRight, slideInFromLeft, fadeIn } from "@/lib/motion";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MinecraftVersion } from "@/types";

/**
 * 下载页面
 * 包含 Minecraft 版本下载和 Java下载
 */
export default function DownloadPage() {
  const { versions, loading, error, refetch } = useMinecraftVersions();
  const [versionFilter, setVersionFilter] = useState<VersionFilter>("release");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedVersion, setSelectedVersion] =
    useState<MinecraftVersion | null>(null);

  const [tab, setTab] = useState<"minecraft" | "java">("minecraft");

  // Java 下载状态
  const [javaDownloading, setJavaDownloading] = useState(false);
  const [javaProgress, setJavaProgress] = useState(0);
  const [javaTargetVersion, setJavaTargetVersion] = useState("17");
  const [javaBasePath, setJavaBasePath] = useState("");
  const [javaMessage, setJavaMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  useEffect(() => {
    invoke<string>("get_java_download_dir").then(setJavaBasePath).catch(() => {});
    const unlisten = listen<number>("java-download-progress", (event) => {
      setJavaProgress(event.payload);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  const handleJavaDownload = async () => {
    if (!javaBasePath) {
      setJavaMessage({ type: "error", text: "下载路径未就绪，请稍候重试" });
      return;
    }
    setJavaDownloading(true);
    setJavaProgress(0);
    setJavaMessage(null);
    try {
      const result = await invoke<{ message: string; java_path: string }>("download_java_runtime", {
        targetVersion: parseInt(javaTargetVersion),
        basePath: javaBasePath,
      });
      setJavaMessage({ type: "success", text: result.message });
      try {
        await invoke("validate_java_path", { javaPath: result.java_path });
      } catch { /* ignore */ }
    } catch (err) {
      setJavaMessage({ type: "error", text: `下载失败: ${err}` });
    } finally {
      setJavaDownloading(false);
      setJavaProgress(0);
    }
  };

  const filteredVersions = useMemo(() => {
    return versions.filter((v) => {
      if (versionFilter !== "all" && v.type !== versionFilter) return false;
      if (
        searchQuery &&
        !v.id.toLowerCase().includes(searchQuery.toLowerCase())
      )
        return false;
      return true;
    });
  }, [versions, versionFilter, searchQuery]);

  return (
    <AnimatePresence mode="wait" initial={false}>
      {selectedVersion ? (
        // 版本详情视图
        <motion.div
          key="detail"
          className="flex h-full flex-col p-4"
          variants={slideInFromRight}
          initial="initial"
          animate="animate"
          exit="exit"
        >
          <VersionDetail
            version={selectedVersion}
            onBack={() => setSelectedVersion(null)}
          />
        </motion.div>
      ) : (
        // 列表视图
        <motion.div
          key="list"
          className="flex h-full flex-col gap-4 p-4"
          variants={slideInFromLeft}
          initial="initial"
          animate="animate"
          exit="exit"
        >
          {/* 页面标题 */}
          <div className="flex items-center gap-3 shrink-0">
            <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
              <Download className="size-5 text-primary" />
            </div>
            <div>
              <h1 className="text-lg font-semibold leading-none">下载中心</h1>
              <p className="mt-1 text-xs text-muted-foreground">
                下载 Minecraft 版本和 Java
              </p>
            </div>
          </div>

          <div className="flex-1 min-h-0 flex flex-col">
            <div className="flex gap-1 shrink-0 rounded-lg bg-muted p-1 w-fit">
              <button
                className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  tab === "minecraft" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setTab("minecraft")}
              >
                Minecraft
              </button>
              <button
                className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  tab === "java" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setTab("java")}
              >
                Java
              </button>
            </div>

            {/* Minecraft 版本下载 */}
            {tab === "minecraft" && <div className="flex-1 min-h-0 flex flex-col gap-4 mt-4">
              <div className="shrink-0">
                <VersionFilterBar
                  filter={versionFilter}
                  onFilterChange={setVersionFilter}
                  searchQuery={searchQuery}
                  onSearchChange={setSearchQuery}
                />
              </div>

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
                ) : error ? (
                  <motion.div
                    key="error"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <AlertCircle className="size-8 text-destructive" />
                    <p className="text-sm">获取版本列表失败</p>
                    <p className="text-xs">{error}</p>
                    <Button variant="outline" size="sm" onClick={refetch} className="mt-2 gap-2">
                      <RefreshCw className="size-3.5" />
                      重试
                    </Button>
                  </motion.div>
                ) : (
                  <motion.div
                    key="list"
                    className="flex-1 min-h-0 flex flex-col"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <VersionList
                      versions={filteredVersions}
                      onSelectVersion={setSelectedVersion}
                    />
                  </motion.div>
                )}
              </AnimatePresence>
            </div>}

            {/* Java 下载 */}
            {tab === "java" && <div className="flex-1 min-h-0 overflow-y-auto mt-4">
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2 text-base">
                    <Coffee className="size-4" />
                    下载 Java
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  {javaMessage && (
                    <div
                      className={`rounded-lg border p-3 text-sm ${
                        javaMessage.type === "error"
                          ? "border-red-500 bg-red-50 text-red-900 dark:bg-red-950 dark:text-red-100"
                          : "border-green-500 bg-green-50 text-green-900 dark:bg-green-950 dark:text-green-100"
                      }`}
                    >
                      {javaMessage.text}
                    </div>
                  )}

                  <div className="space-y-2">
                    <Label htmlFor="java-version">目标 Java 版本</Label>
                    <Input
                      id="java-version"
                      type="number"
                      placeholder="17"
                      value={javaTargetVersion}
                      onChange={(e) => setJavaTargetVersion(e.target.value)}
                      disabled={javaDownloading}
                    />
                    <p className="text-xs text-muted-foreground">
                      常用版本: 8, 11, 17, 21 | MC 1.17+ 需要 Java 17, MC 1.20.5+ 需要 Java 21
                    </p>
                  </div>

                  {javaDownloading && (
                    <div className="space-y-2">
                      <div className="flex items-center justify-between text-sm">
                        <span>下载进度</span>
                        <span>{javaProgress.toFixed(1)}%</span>
                      </div>
                      <Progress value={javaProgress} />
                    </div>
                  )}

                  <Button
                    onClick={handleJavaDownload}
                    disabled={javaDownloading || !javaTargetVersion || !javaBasePath}
                    className="w-full"
                  >
                    <Download className={`mr-2 size-4 ${javaDownloading ? "animate-pulse" : ""}`} />
                    {javaDownloading ? "下载中..." : "开始下载"}
                  </Button>
                </CardContent>
              </Card>
            </div>}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
