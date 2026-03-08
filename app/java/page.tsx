"use client"

import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { Coffee, Download, CheckCircle2, AlertCircle, X, Search, Save } from "lucide-react"
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

export default function JavaPage() {
  const [versions, setVersions] = useState<JavaVersion[]>([])
  const [installations, setInstallations] = useState<JavaInstallation[]>([])
  const [loading, setLoading] = useState(false)
  const [searching, setSearching] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [progress, setProgress] = useState(0)
  const [targetVersion, setTargetVersion] = useState("17")
  const [basePath, setBasePath] = useState("")
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null)

  useEffect(() => {
    loadJavaVersions()
    // 不自动搜索，等待用户点击按钮

    const unlisten = listen<number>("java-download-progress", (event) => {
      setProgress(event.payload)
    })

    return () => {
      unlisten.then((fn) => fn())
    }
  }, [])

  const loadJavaVersions = async () => {
    setLoading(true)
    try {
      const result = await invoke<JavaVersion[]>("get_java_versions")
      setVersions(result)
    } catch (error) {
      setMessage({ type: "error", text: `获取Java版本失败: ${error}` })
    } finally {
      setLoading(false)
    }
  }

  const searchInstalledJava = async () => {
    setSearching(true)
    setMessage(null)
    try {
      const result = await invoke<JavaInstallation[]>("search_java_installations")
      setInstallations(result)

      // 保存到配置
      if (result.length > 0) {
        await saveJavaInstallations(result)
      } else {
        setMessage({ type: "error", text: "未找到已安装的Java，请手动下载或安装" })
      }
    } catch (error) {
      console.error("搜索Java失败:", error)
      setMessage({ type: "error", text: `搜索Java失败: ${error}` })
    } finally {
      setSearching(false)
    }
  }

  const saveJavaInstallations = async (javas: JavaInstallation[]) => {
    try {
      const config = await invoke<any>("get_launcher_paths_config")

      // 构建Java安装信息映射
      const javaInstallations: Record<string, JavaInstallation> = {}
      const javaPaths: string[] = []

      javas.forEach((java) => {
        // 保存完整的安装信息，使用 "java{version}" 作为key
        javaInstallations[`java${java.major_version}`] = java
        // 添加到 java_paths 数组（启动界面使用）
        if (!javaPaths.includes(java.path)) {
          javaPaths.push(java.path)
        }
      })

      // 合并现有的java_paths，避免覆盖用户手动添加的路径
      const existingPaths = config.java_paths || []
      const mergedPaths = [...new Set([...javaPaths, ...existingPaths])]

      // 如果没有选中的Java路径，自动选择最新版本（版本号最高的）
      let selectedJavaPath = config.selected_java_path || ""
      if (!selectedJavaPath && javas.length > 0) {
        // javas已经按major_version降序排列
        selectedJavaPath = javas[0].path
      }

      await invoke("save_launcher_paths_config", {
        config: {
          ...config,
          java_installations: javaInstallations,
          java_paths: mergedPaths,
          selected_java_path: selectedJavaPath,
        }
      })

      setMessage({
        type: "success",
        text: `已保存 ${javas.length} 个Java到启动配置，可在启动界面直接使用`
      })
    } catch (error) {
      console.error("保存Java配置失败:", error)
      setMessage({
        type: "error",
        text: `保存配置失败: ${error}`
      })
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
      const result = await invoke<string>("download_java_runtime", {
        targetVersion: parseInt(targetVersion),
        basePath,
      })

      setMessage({ type: "success", text: result })

      // 重新搜索Java
      await searchInstalledJava()
    } catch (error) {
      setMessage({ type: "error", text: `下载失败: ${error}` })
    } finally {
      setDownloading(false)
      setProgress(0)
    }
  }

  const getVersionBadgeColor = (majorVersion: number) => {
    if (majorVersion >= 21) return "bg-purple-500"
    if (majorVersion >= 17) return "bg-blue-500"
    if (majorVersion >= 11) return "bg-green-500"
    if (majorVersion >= 8) return "bg-yellow-500"
    return "bg-gray-500"
  }

  return (
    <div className="flex h-full flex-col gap-6 p-6">
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
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={() => setMessage(null)}
          >
            <X className="size-4" />
          </Button>
        </div>
      )}

      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <div>
                <CardTitle>已安装的Java</CardTitle>
                <CardDescription>
                  系统中已安装的Java运行时
                </CardDescription>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={searchInstalledJava}
                disabled={searching}
              >
                {searching ? (
                  <>
                    <Search className="mr-2 size-4 animate-spin" />
                    搜索中...
                  </>
                ) : (
                  <>
                    <Search className="mr-2 size-4" />
                    重新搜索
                  </>
                )}
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
                {installations.map((installation, index) => (
                  <div
                    key={index}
                    className="rounded-lg border p-4 space-y-2"
                  >
                    <div className="flex items-start justify-between">
                      <div className="space-y-1">
                        <div className="flex items-center gap-2">
                          <Badge className={getVersionBadgeColor(installation.major_version)}>
                            Java {installation.major_version}
                          </Badge>
                          <span className="text-sm font-medium">
                            {installation.vendor}
                          </span>
                        </div>
                        <p className="text-xs text-muted-foreground">
                          版本: {installation.version}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          架构: {installation.architecture}
                        </p>
                      </div>
                      <CheckCircle2 className="size-5 text-green-500 flex-shrink-0" />
                    </div>
                    <div className="pt-2 border-t">
                      <p className="text-xs text-muted-foreground break-all">
                        {installation.path}
                      </p>
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

        <Card>
          <CardHeader>
            <CardTitle>下载Java</CardTitle>
            <CardDescription>
              输入目标Java版本号，系统会自动选择最合适的版本
            </CardDescription>
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
              <p className="text-xs text-muted-foreground">
                常用版本: 8, 11, 17, 21
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="path">下载路径</Label>
              <Input
                id="path"
                placeholder="C:\Java"
                value={basePath}
                onChange={(e) => setBasePath(e.target.value)}
                disabled={downloading}
              />
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
              {downloading ? (
                <>
                  <Download className="mr-2 size-4 animate-pulse" />
                  下载中...
                </>
              ) : (
                <>
                  <Download className="mr-2 size-4" />
                  开始下载
                </>
              )}
            </Button>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>可用版本</CardTitle>
            <CardDescription>
              当前系统支持的Java运行时版本
            </CardDescription>
          </CardHeader>
          <CardContent>
            {loading ? (
              <div className="flex items-center justify-center py-8">
                <div className="size-8 animate-spin rounded-full border-4 border-primary border-t-transparent" />
              </div>
            ) : versions.length > 0 ? (
              <div className="space-y-2 max-h-[300px] overflow-y-auto">
                {versions.map((version, index) => (
                  <div
                    key={index}
                    className="flex items-center justify-between rounded-lg border p-3"
                  >
                    <div>
                      <p className="font-medium">{version.name}</p>
                      <p className="text-sm text-muted-foreground">
                        {version.version}
                      </p>
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
