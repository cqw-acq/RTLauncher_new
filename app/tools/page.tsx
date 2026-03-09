"use client"

import { useState, useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { Wrench, Search, CheckCircle2, AlertCircle, Coffee } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"

interface JavaInstallation {
  path: string
  version: string
  major_version: number
  vendor: string
  architecture: string
  java_type: string
}

function getVersionBadgeColor(majorVersion: number): string {
  if (majorVersion >= 21) return "bg-purple-500"
  if (majorVersion >= 17) return "bg-blue-500"
  if (majorVersion >= 11) return "bg-green-500"
  if (majorVersion >= 8) return "bg-yellow-500"
  return "bg-gray-500"
}

export default function ToolsPage() {
  const [installations, setInstallations] = useState<JavaInstallation[]>([])
  const [searching, setSearching] = useState(false)

  useEffect(() => {
    searchInstalledJava()
  }, [])

  const searchInstalledJava = async () => {
    setSearching(true)
    try {
      const result = await invoke<JavaInstallation[]>("search_java_installations")
      setInstallations(result)

      if (result.length > 0) {
        const config = await invoke<any>("get_launcher_paths_config")
        const javaInstallations: Record<string, JavaInstallation> = {}
        const javaPaths: string[] = []
        for (const java of result) {
          javaInstallations[java.path] = java
          if (!javaPaths.includes(java.path)) javaPaths.push(java.path)
        }
        const existingPaths: string[] = config.java_paths ?? []
        const mergedPaths = [...new Set([...javaPaths, ...existingPaths])]
        await invoke("save_launcher_paths_config", {
          config: {
            ...config,
            java_installations: javaInstallations,
            java_paths: mergedPaths,
            selected_java_path: config.selected_java_path || (result[0]?.path ?? ""),
          },
        })
      }
    } catch { /* ignore */ }
    setSearching(false)
  }

  return (
    <div className="flex h-full flex-col gap-6 p-6 overflow-y-auto">
      <div className="flex items-center gap-3">
        <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
          <Wrench className="size-5 text-primary" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none">工具</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            管理 Java和其他工具
          </p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Coffee className="size-4" />
              <div>
                <CardTitle className="text-base">已安装的 Java</CardTitle>
                <CardDescription>系统中已安装的 Java</CardDescription>
              </div>
            </div>
            <Button variant="outline" size="sm" onClick={searchInstalledJava} disabled={searching}>
              <Search className={`mr-2 size-4 ${searching ? "animate-spin" : ""}`} />
              {searching ? "扫描中..." : "重新扫描"}
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
                        <Badge variant="outline" className="text-xs">{inst.java_type}</Badge>
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
              <p>未找到已安装的 Java</p>
              <p className="text-xs mt-1">可前往下载页面下载 Java</p>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
