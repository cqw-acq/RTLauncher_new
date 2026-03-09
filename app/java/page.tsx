"use client"

import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { open as openDialog } from "@tauri-apps/plugin-dialog"
import { Coffee, Download, CheckCircle2, AlertCircle, X, Search, FolderOpen } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"

interface JavaVersion {
  name: string
  version: string
}

interface JavaInstallation {
  path: string
  version: string
  major_version: number
  vendor: string
  architecture: string
}

type Message = { type: "success" | "error"; text: string }

async function saveJavaInstallations(javas: JavaInstallation[]): Promise<void> {
  const config = await invoke<any>("get_launcher_paths_config")

  const javaInstallations: Record<string, JavaInstallation> = {}
  const javaPaths: string[] = []

  for (const java of javas) {
    javaInstallations[java.path] = java
    if (!javaPaths.includes(java.path)) javaPaths.push(java.path)
  }

  const existingPaths: string[] = config.java_paths ?? []
  const mergedPaths = [...new Set([...javaPaths, ...existingPaths])]

  const selectedJavaPath = config.selected_java_path || (javas[0]?.path ?? "")

  await invoke("save_launcher_paths_config", {
    config: {
      ...config,
      java_installations: javaInstallations,
      java_paths: mergedPaths,
      selected_java_path: selectedJavaPath,
    },
  })
}

function getVersionBadgeColor(majorVersion: number): string {
  if (majorVersion >= 21) return "bg-purple-500"
  if (majorVersion >= 17) return "bg-blue-500"
  if (majorVersion >= 11) return "bg-green-500"
  if (majorVersion >= 8) return "bg-yellow-500"
  return "bg-gray-500"
}

export default function JavaPage() {
  const [versions, setVersions] = useState<JavaVersion[]>([])
  const [installations, setInstallations] = useState<JavaInstallation[]>([])
  const [loadingVersions, setLoadingVersions] = useState(false)
  const [searching, setSearching] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState(0)
  const [targetVersion, setTargetVersion] = useState("17")
  const [basePath, setBasePath] = useState("")
  const [message, setMessage] = useState<Message | null>(null)

  useEffect(() => {
    loadJavaVersions()
    searchInstalledJava()

    const unlisten = listen<number>("java-download-progress", (event) => {
      setProgress(event.payload)
    })
    return () => { unlisten.then((fn) => fn()) }
  }, [])

  const loadJavaVersions = async () => {
    setLoadingVersions(true)
    try {
      const result = await invoke<JavaVersion[]>("get_java_versions")
      setVersions(result)
    } catch (error) {
      setMessage({ type: "error", text: `获取Java版本失败: ${error}` })
    } finally {
      setLoadingVersions(false)
    }
  }

  const searchInstalledJava = async () => {
    setSearching(true)
    setMessage(null)
    try {
      const result = await invoke<JavaInstallation[]>("search_java_installations")
      setInstallations(result)

      if (result.length === 0) {
        setMessage({ type: "error", text: "未找到已安装的Java，请手动下载或安装" })
        return
      }

      await saveJavaInstallations(result)
      setMessage({
        type: "success",
        text: `已保存 ${result.length} 个Java到启动配置，可在启动界面直接使用`,
      })
    } catch (error) {
      setMessage({ type: "error", text: `搜索Java失败: ${error}` })
    } finally {
      setSearching(false)
    }
  }

  const handleBrowse = async () => {
    try {
      const selected = await openDialog({ directory: true, multiple: false })
      if (selected) setBasePath(selected as string)
    } catch {
      // dialog cancelled
    }
  }

  const handleDownload = async () => {
    if (!basePath) {
      setMessage({ type: "error", text: "请输入下载路径" })
      return
    }

    setDownloading(true)
    setProgress(0)
    setMessage(null)

    try {
      const result = await invoke<{ message: string; java_path: string }>("download_java_runtime", {
        targetVersion: parseInt(targetVersion),
        basePath,
      })
      setMessage({ type: "success", text: result.message })

      try {
        const newInst = await invoke<JavaInstallation>("validate_java_path", {
          javaPath: result.java_path,
        })
        setInstallations((prev) => {
          if (prev.some((i) => i.path === newInst.path)) return prev
          const updated = [...prev, newInst].sort((a, b) => b.major_version - a.major_version)
          saveJavaInstallations(updated)
          return updated
        })
      } catch {
        // 直接获取失败，回退到全量搜索
        await searchInstalledJava()
      }
    } catch (error) {
      setMessage({ type: "error", text: `下载失败: ${error}` })
    } finally {
      setDownloading(false)
      setProgress(0)
    }
  }

  return (
    <div className="flex h-full flex-col gap-6 p-6 overflow-y-auto">
      <div className="flex items-center gap-3">
        <Coffee className="size-8" />
        <div>
          <h1 className="text-2xl font-bold">Java管理</h1>
          <p className="text-sm text-muted-foreground">
            自动搜索、下载和管理Minecraft所需的Java运行时
          </p>
        </div>
      </div>

      {message && (
        <div
          className={`flex items-center justify-between rounded-lg border p-4 ${
            message.type === "error"
              ? "border-red-500 bg-red-50 text-red-900 dark:bg-red-950 dark:text-red-100"
              : "border-green-500 bg-green-50 text-green-900 dark:bg-green-950 dark:text-green-100"
          }`}
        >
          <div className="flex items-center gap-2">
            {message.type === "error" ? (
              <AlertCircle className="size-5" />
            ) : (
              <CheckCircle2 className="size-5" />
            )}
            <p className="text-sm">{message.text}</p>
          </div>
          <Button variant="ghost" size="icon" className="size-6" onClick={() => setMessage(null)}>
            <X className="size-4" />
          </Button>
        </div>
      )}

      <div className="grid gap-6 md:grid-cols-2">
        {/* 已安装的Java */}
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <div>
                <CardTitle>已安装的Java</CardTitle>
                <CardDescription>系统中已安装的Java运行时</CardDescription>
              </div>
              <Button variant="outline" size="sm" onClick={searchInstalledJava} disabled={searching}>
                <Search className={`mr-2 size-4 ${searching ? "animate-spin" : ""}`} />
                {searching ? "搜索中..." : "重新搜索"}
              </Button>
            </div>
          </CardHeader>
          <CardContent>
            {searching ? (
              <div className="flex items-center justify-center py-8">
                <div className="size-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
              </div>
            ) : installations.length > 0 ? (
              <div className="space-y-3 max-h-[500px] overflow-y-auto">
                {installations.map((inst, index) => (
                  <div key={index} className="rounded-lg border p-4 space-y-2">
                    <div className="flex items-start justify-between">
                      <div className="space-y-1">
                        <div className="flex items-center gap-2">
                          <Badge className={getVersionBadgeColor(inst.major_version)}>
                            Java {inst.major_version}
                          </Badge>
                          <span className="text-sm font-medium">{inst.vendor}</span>
                        </div>
                        <p className="text-xs text-muted-foreground">版本: {inst.version}</p>
                        <p className="text-xs text-muted-foreground">架构: {inst.architecture}</p>
                      </div>
                      <CheckCircle2 className="size-5 text-green-500 flex-shrink-0" />
                    </div>
                    <div className="pt-2 border-t">
                      <p className="text-xs text-muted-foreground break-all">{inst.path}</p>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
                <AlertCircle className="mb-2 size-8" />
                <p>未找到已安装的Java</p>
                <p className="text-xs mt-1">请下载或手动安装Java</p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* 下载Java */}
        <Card>
          <CardHeader>
            <CardTitle>下载Java</CardTitle>
            <CardDescription>输入目标Java版本号，系统会自动选择最合适的版本</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="version">目标Java版本</Label>
              <Input
                id="version"
                type="number"
                placeholder="17"
                value={targetVersion}
                onChange={(e) => setTargetVersion(e.target.value)}
                disabled={downloading}
              />
              <p className="text-xs text-muted-foreground">常用版本: 8, 11, 17, 21</p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="path">下载路径</Label>
              <div className="flex gap-2">
                <Input
                  id="path"
                  placeholder="/path/to/java"
                  value={basePath}
                  onChange={(e) => setBasePath(e.target.value)}
                  disabled={downloading}
                  className="flex-1"
                />
                <Button variant="outline" size="icon" onClick={handleBrowse} disabled={downloading} title="选择目录">
                  <FolderOpen className="size-4" />
                </Button>
              </div>
            </div>

            {downloading && (
              <div className="space-y-2">
                <div className="flex items-center justify-between text-sm">
                  <span>下载进度</span>
                  <span>{progress.toFixed(1)}%</span>
                </div>
                <Progress value={progress} />
              </div>
            )}

            <Button
              onClick={handleDownload}
              disabled={downloading || !targetVersion || !basePath}
              className="w-full"
            >
              <Download className={`mr-2 size-4 ${downloading ? "animate-pulse" : ""}`} />
              {downloading ? "下载中..." : "开始下载"}
            </Button>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        {/* 可用版本 */}
        <Card>
          <CardHeader>
            <CardTitle>可用版本</CardTitle>
            <CardDescription>当前系统支持的Java运行时版本</CardDescription>
          </CardHeader>
          <CardContent>
            {loadingVersions ? (
              <div className="flex items-center justify-center py-8">
                <div className="size-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
              </div>
            ) : versions.length > 0 ? (
              <div className="space-y-2 max-h-[300px] overflow-y-auto">
                {versions.map((version, index) => (
                  <div key={index} className="flex items-center justify-between rounded-lg border p-3">
                    <div>
                      <p className="font-medium">{version.name}</p>
                      <p className="text-sm text-muted-foreground">{version.version}</p>
                    </div>
                    <CheckCircle2 className="size-5 text-green-500" />
                  </div>
                ))}
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
                <AlertCircle className="mb-2 size-8" />
                <p>暂无可用版本</p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* 使用说明 */}
        <Card>
          <CardHeader>
            <CardTitle>使用说明</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm text-muted-foreground">
            <p>• 点击"重新搜索"自动扫描系统中的Java安装</p>
            <p>• 搜索到的Java会自动保存到启动器配置中</p>
            <p>• 可以手动下载指定版本的Java运行时</p>
            <p>• 支持识别Java版本、供应商和架构信息</p>
            <p>• 推荐版本: MC 1.17+ 使用Java 17, MC 1.20.5+ 使用Java 21</p>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
