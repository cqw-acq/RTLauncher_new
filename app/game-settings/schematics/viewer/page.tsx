"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { Upload, List, Download, Info, FileBox, Settings, X, RotateCcw, Maximize2, Minimize2, ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useI18n } from "@/components/i18n/use-i18n";
import type { AppLanguage } from "@/components/settings/settings-provider";
import { useRouter } from "next/navigation";

declare global {
  interface Window {
    deepslate: any;
    glMatrix: any;
    deepslateResources: any;
    loadDeepslateResources: (image: HTMLImageElement) => any;
    readLitematicFromNBTData: (nbtdata: any) => any;
    getMaterialList: (litematic: any) => Record<string, number>;
    structureFromLitematic: (litematic: any, y_min?: number, y_max?: number) => any;
    setStructure: (structure: any, reset_view?: boolean) => void;
    render: () => void;
    createRenderCanvas: (canvas: HTMLCanvasElement) => void;
    destroyRenderCanvas: () => void;
    structureLitematic: any;
  }
}

function base64ToArrayBuffer(base64: string): ArrayBuffer {
  const binaryString = atob(base64);
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes.buffer;
}

function loadScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const existing = document.querySelector(`script[src="${src}"]`);
    if (existing) { resolve(); return; }
    const script = document.createElement("script");
    script.src = src;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error(`Failed to load ${src}`));
    document.head.appendChild(script);
  });
}

async function loadScriptAsUMD(src: string, globalName: string): Promise<void> {
  const resp = await fetch(src);
  if (!resp.ok) throw new Error(`HTTP ${resp.status} fetching ${src}`);
  const code = await resp.text();
  const factory = new Function("module", "exports", "define", "global", "window", "self", `${code}\n; return module.exports;`);
  const module = { exports: {} as any };
  const exports = module.exports;
  const define = (deps: any, factoryFn: any) => {
    const args = deps.map((d: string) => d === "exports" ? exports : (window as any)[d.replace(/^\.\//, "")] || {});
    module.exports = factoryFn ? factoryFn(...args) : deps;
  };
  (define as any).amd = true;
  const ctx = typeof window !== "undefined" ? window : typeof self !== "undefined" ? self : globalThis;
  const result = factory.call(ctx, module, exports, define, ctx, window || ctx, self || ctx);
  const exposed = result && Object.keys(result).length ? result : (module.exports && Object.keys(module.exports).length ? module.exports : null);
  if (exposed && !(window as any)[globalName]) {
    (window as any)[globalName] = exposed;
  }
}

export default function SchematicViewerPage() {
  const router = useRouter();
  const { language } = useI18n();
  const L = <T extends { [k in AppLanguage]: string }>(obj: T): string => obj[language] ?? obj["zh-CN"] ?? obj["en-US"];
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const atlasImgRef = useRef<HTMLImageElement>(null);
  const autoLoadAttemptedRef = useRef(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const [scriptsLoaded, setScriptsLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [resourcesLoaded, setResourcesLoaded] = useState(false);
  const [localFileStatus, setLocalFileStatus] = useState<Record<string, boolean>>({});
  const [schematicInfo, setSchematicInfo] = useState<any>(null);
  const [blockCounts, setBlockCounts] = useState<Record<string, number> | null>(null);
  const [showMaterialList, setShowMaterialList] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [minLayer, setMinLayer] = useState(0);
  const [maxLayer, setMaxLayer] = useState(0);
  const [maxY, setMaxY] = useState(0);
  const [fileName, setFileName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const LOCAL_FILES = [
      { name: "assets.js", path: "/schematic-viewer/assets.js" },
      { name: "deepslate.js", path: "/schematic-viewer/deepslate.js" },
      { name: "gl-matrix-min.js", path: "/schematic-viewer/gl-matrix-min.js" },
    ];

    (async function checkLocalFiles() {
      const status: Record<string, boolean> = {};
      for (const f of LOCAL_FILES) {
        try {
          const r = await fetch(f.path, { method: "HEAD" });
          status[f.name] = r.ok;
        } catch {
          status[f.name] = false;
        }
      }
      if (!cancelled) setLocalFileStatus(status);
    })();

    async function init() {
      try {
        const CDN_URLS = [
          "https://cdn.jsdelivr.net/npm",
          "https://unpkg.com",
          "https://unpkg.zhimg.com",
        ];

        async function fetchWithFallback(path: string, localPath?: string): Promise<string> {
          if (localPath) {
            try {
              const resp = await fetch(localPath);
              if (resp.ok) return await resp.text();
            } catch {}
          }
          for (const base of CDN_URLS) {
            try {
              const resp = await fetch(`${base}${path}`);
              if (resp.ok) return await resp.text();
            } catch {}
          }
          throw new Error(`All sources (local + CDN) failed for ${path}`);
        }

        async function loadScriptFallback(path: string, localPath?: string, ensureGlobal?: string): Promise<void> {
          if (localPath) {
            try {
              if (ensureGlobal) {
                await loadScriptAsUMD(localPath, ensureGlobal);
              } else {
                await loadScript(localPath);
              }
              return;
            } catch {}
          }
          for (const base of CDN_URLS) {
            try {
              if (ensureGlobal) {
                await loadScriptAsUMD(`${base}${path}`, ensureGlobal);
              } else {
                await loadScript(`${base}${path}`);
              }
              return;
            } catch {}
          }
          throw new Error(`All sources (local + CDN) failed for ${path}`);
        }

        await Promise.all([
          loadScriptFallback("/deepslate@0.10.1/dist/deepslate.js", "/schematic-viewer/deepslate.js", "deepslate"),
          loadScriptFallback("/gl-matrix@3.4.3/gl-matrix-min.js", "/schematic-viewer/gl-matrix-min.js", "glMatrix"),
        ]);
        if (cancelled) return;

        if (typeof (window as any).glMatrix === "undefined") {
          throw new Error("glMatrix failed to initialize (window.glMatrix is undefined) - please check /schematic-viewer/gl-matrix-min.js");
        }
        if (typeof (window as any).deepslate === "undefined") {
          throw new Error("deepslate failed to initialize (window.deepslate is undefined) - please check /schematic-viewer/deepslate.js");
        }

        const assetsText = await fetchWithFallback("/deepslate@0.10.1/dist/assets.js", "/schematic-viewer/assets.js");
        if (cancelled) return;

        if (typeof (window as any).assets === "undefined") {
          if (!(window as any).__dvAssetsGuard) {
            (window as any).__dvAssetsGuard = true;
            const sanitized = assetsText
              .replace(/^\s*(?:const|let|var)\s+assets\s*=/, "var assets =");
            const factory = new Function("window", "globalThis", "self",
              `${sanitized}
if (typeof assets !== "undefined") { window.assets = assets; }`);
            try {
              factory(window, window, window);
            } catch (e) {
              if (!(e instanceof SyntaxError && /assets|already been declared|Identifier/.test(e.message || ""))) {
                throw e;
              }
            }
          }
        }

        if (typeof (window as any).assets === "undefined") {
          throw new Error("Failed to evaluate assets.js (window.assets is undefined)");
        }

        await loadScript("/schematic-viewer/opaque.js");
        await loadScript("/schematic-viewer/deepslate-helpers.js");
        await loadScript("/schematic-viewer/litematic-utils.js");
        await loadScript("/schematic-viewer/viewer.js");
        if (cancelled) return;

        setScriptsLoaded(true);
      } catch (e) {
        console.error("[schematic-viewer] Failed to load scripts:", e);
        setLoadError(e instanceof Error ? e.message : String(e));
      }
    }

    init();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!scriptsLoaded) return;
    const image = atlasImgRef.current;
    if (!image) return;

    function onLoad() {
      if (window.loadDeepslateResources) {
        window.loadDeepslateResources(image!);
        setResourcesLoaded(true);
      }
    }

    if (image.complete) {
      onLoad();
    } else {
      image.addEventListener("load", onLoad);
      return () => image.removeEventListener("load", onLoad);
    }
  }, [scriptsLoaded]);

  useEffect(() => {
    if (!resourcesLoaded || !canvasRef.current) return;
    const canvas = canvasRef.current;

    const resizeCanvas = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const w = Math.max(1, Math.round(rect.width));
      const h = Math.max(1, Math.round(rect.height));
      if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
        canvas.width = w * dpr;
        canvas.height = h * dpr;
      }
    };
    resizeCanvas();
    window.addEventListener("resize", resizeCanvas);
    const ro = new ResizeObserver(resizeCanvas);
    if (canvas.parentElement) ro.observe(canvas.parentElement);

    if (window.createRenderCanvas) {
      window.createRenderCanvas(canvas);
    }

    return () => {
      window.removeEventListener("resize", resizeCanvas);
      ro.disconnect();
      if (window.destroyRenderCanvas) {
        window.destroyRenderCanvas();
      }
    };
  }, [resourcesLoaded]);

  const processFileBuffer = useCallback(
    (buffer: ArrayBuffer, name: string) => {
      if (!window.deepslateResources) {
        alert("Resources not loaded yet. Please wait.");
        return false;
      }

      setIsLoading(true);

      try {
        const nbtdata = window.deepslate.readNbt(new Uint8Array(buffer));
        const litematic = window.readLitematicFromNBTData(nbtdata);
        window.structureLitematic = litematic;

        const region0 = litematic.regions[0];
        const blocks = region0.blocks;
        const height = blocks[0].length;
        const mY = height;

        setMaxY(mY);
        setMinLayer(0);
        setMaxLayer(mY - 1);
        setFileName(name);

        const counts = window.getMaterialList(litematic);
        setBlockCounts(counts);

        let infoName = name.replace(/\.(litematic|schem|schematic|nbt)$/i, "");
        let infoDesc = "";
        let infoAuthor = "-";
        let infoDate = "-";
        try {
          const nbtRoot = nbtdata.value;
          infoDesc = nbtRoot.Metadata?.value?.Description?.value || "";
          infoAuthor = nbtRoot.Metadata?.value?.Author?.value || "-";
          const rawDate = nbtRoot.Metadata?.value?.TimeCreated?.value;
          infoDate = rawDate ? new Date(rawDate).toLocaleString() : "-";
        } catch {}

        setSchematicInfo({
          name: infoName,
          description: infoDesc,
          author: infoAuthor,
          date: infoDate,
          size: `${region0.width} \u00d7 ${region0.height} \u00d7 ${region0.depth}`,
          blocks: Object.values(counts).reduce((a: number, b: number) => a + b, 0),
        });

        const structure = window.structureFromLitematic(litematic);
        window.setStructure(structure, true);
        return true;
      } catch (e) {
        console.error("[schematic-viewer] Load failed:", e);
        alert("Failed to load file:\n" + (e instanceof Error ? e.message : String(e)));
        return false;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const handleFileLoad = useCallback(
    async (file: File) => {
      const reader = new FileReader();
      reader.readAsArrayBuffer(file);
      reader.onload = () => {
        if (reader.result) {
          processFileBuffer(reader.result as ArrayBuffer, file.name);
        }
      };
      reader.onerror = () => {
        alert("Failed to read file");
      };
    },
    [processFileBuffer]
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const file = e.dataTransfer.files[0];
      if (file) handleFileLoad(file);
    },
    [handleFileLoad]
  );

  const handleLayerChange = useCallback(
    (min: number, max: number) => {
      if (!window.structureLitematic) return;
      if (min > max) return;
      setMinLayer(min);
      setMaxLayer(max);
      const structure = window.structureFromLitematic(window.structureLitematic, min, max + 1);
      window.setStructure(structure);
    },
    []
  );

  const resetView = useCallback(() => {
    if (!window.structureLitematic) return;
    const structure = window.structureFromLitematic(window.structureLitematic, minLayer, maxLayer + 1);
    window.setStructure(structure, true);
  }, [minLayer, maxLayer]);

  const exitToSchematics = useCallback(() => {
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    }
    router.push("/game-settings/schematics");
  }, [router]);

  const toggleFullscreen = useCallback(() => {
    if (!containerRef.current) return;
    if (!document.fullscreenElement) {
      containerRef.current.requestFullscreen?.().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
  }, []);

  useEffect(() => {
    const onFsChange = () => setIsFullscreen(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", onFsChange);
    return () => document.removeEventListener("fullscreenchange", onFsChange);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (showSettings) { setShowSettings(false); return; }
      if (showMaterialList) { setShowMaterialList(false); return; }
      if (document.fullscreenElement) { document.exitFullscreen().catch(() => {}); return; }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [showSettings, showMaterialList]);

  const unloadSchematic = useCallback(() => {
    if (window.destroyRenderCanvas) {
      window.destroyRenderCanvas();
    }
    if (canvasRef.current && window.createRenderCanvas) {
      const canvas = canvasRef.current;
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = Math.max(1, Math.round(rect.width)) * dpr;
      canvas.height = Math.max(1, Math.round(rect.height)) * dpr;
      window.createRenderCanvas(canvas);
    }
    window.structureLitematic = null;
    setSchematicInfo(null);
    setBlockCounts(null);
    setFileName("");
    setShowMaterialList(false);
  }, []);

  const exportCSV = useCallback(() => {
    if (!blockCounts) return;
    const csvContent = Object.entries(blockCounts)
      .sort(([, a], [, b]) => b - a)
      .map(([key, val]) => `${key},${val}`)
      .join("\n");
    const blob = new Blob([csvContent], { type: "text/csv" });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "MaterialList.csv";
    a.click();
    window.URL.revokeObjectURL(url);
  }, [blockCounts]);

  useEffect(() => {
    if (!resourcesLoaded || autoLoadAttemptedRef.current) return;
    let key: string | null = null;
    try {
      if (typeof window !== "undefined" && window.location && window.location.search) {
        const params = new URLSearchParams(window.location.search);
        key = params.get("k");
      }
    } catch {}
    if (!key) return;
    autoLoadAttemptedRef.current = true;

    (async () => {
      try {
        const raw = sessionStorage.getItem(key!);
        if (!raw) return;
        const obj = JSON.parse(raw);
        if (!obj || !obj.b64 || !obj.name) return;
        const buffer = base64ToArrayBuffer(obj.b64);
        processFileBuffer(buffer, obj.name);
        try {
          sessionStorage.removeItem(key!);
        } catch {}
      } catch (e) {
        console.error("[schematic-viewer] auto load from sessionStorage failed:", e);
      }
    })();
  }, [resourcesLoaded, processFileBuffer]);

  return (
    <div
      ref={containerRef}
      className="relative w-full h-full overflow-hidden bg-black"
      onDragOver={(e) => e.preventDefault()}
      onDrop={handleDrop}
    >
      <img
        ref={atlasImgRef}
        id="atlas"
        src="/schematic-viewer/atlas.png"
        alt="atlas"
        crossOrigin="anonymous"
        hidden
      />

      <canvas
        ref={canvasRef}
        className="absolute inset-0 w-full h-full"
        style={{ cursor: "grab", zIndex: 0 }}
      />

      {loadError && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm overflow-y-auto py-8">
          <div className="text-center max-w-lg p-6">
            <div className="text-red-400 text-lg mb-3">
              {L({ "zh-CN": "资源加载失败", "en-US": "Resource Load Failed" })}
            </div>
            <div className="text-zinc-400 text-sm mb-4 text-left space-y-2">
              <p>
                {L({
                  "zh-CN": "由于 CDN 不可访问，请将以下 3 个文件手动复制到",
                  "en-US": "CDN is not accessible. Please manually copy the following 3 files to",
                })}
                <code className="bg-zinc-800 px-1.5 py-0.5 rounded text-blue-400 font-mono text-xs">public/schematic-viewer/</code>
              </p>
              <ol className="list-decimal list-inside space-y-1 text-xs text-zinc-300 pl-2">
                <li>
                  <code className="bg-zinc-800 px-1 rounded">assets.js</code>
                  <span className="text-zinc-500 ml-2">← F:\litematic-viewer\resource\assets.js</span>
                  {localFileStatus["assets.js"] ? (
                    <span className="text-green-500 ml-2">({L({ "zh-CN": "✓ 已就绪", "en-US": "✓ Ready" })})</span>
                  ) : (
                    <span className="text-red-500 ml-2">({L({ "zh-CN": "✗ 缺失", "en-US": "✗ Missing" })})</span>
                  )}
                </li>
                <li>
                  <code className="bg-zinc-800 px-1 rounded">deepslate.js</code>
                  <span className="text-zinc-500 ml-2">← 下载 https://cdn.jsdelivr.net/npm/deepslate@0.10.1/dist/deepslate.js</span>
                  {localFileStatus["deepslate.js"] ? (
                    <span className="text-green-500 ml-2">({L({ "zh-CN": "✓ 已就绪", "en-US": "✓ Ready" })})</span>
                  ) : (
                    <span className="text-red-500 ml-2">({L({ "zh-CN": "✗ 缺失", "en-US": "✗ Missing" })})</span>
                  )}
                </li>
                <li>
                  <code className="bg-zinc-800 px-1 rounded">gl-matrix-min.js</code>
                  <span className="text-zinc-500 ml-2">← 下载 https://cdn.jsdelivr.net/npm/gl-matrix@3.4.3/gl-matrix-min.js</span>
                  {localFileStatus["gl-matrix-min.js"] ? (
                    <span className="text-green-500 ml-2">({L({ "zh-CN": "✓ 已就绪", "en-US": "✓ Ready" })})</span>
                  ) : (
                    <span className="text-red-500 ml-2">({L({ "zh-CN": "✗ 缺失", "en-US": "✗ Missing" })})</span>
                  )}
                </li>
              </ol>
              <p className="text-xs text-zinc-500 pt-2">
                {L({
                  "zh-CN": "复制完成后刷新页面即可",
                  "en-US": "Refresh the page after copying",
                })}
              </p>
            </div>
            <p className="text-zinc-600 text-xs border-t border-zinc-800 pt-3 mt-3">{loadError}</p>
          </div>
        </div>
      )}

      {!loadError && !resourcesLoaded && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div className="text-center">
            <div className="w-12 h-12 border-4 border-zinc-600 border-t-blue-500 rounded-full animate-spin mx-auto mb-4" />
            <p className="text-zinc-300 text-sm">
              {L({ "zh-CN": "加载资源中...", "en-US": "Loading resources..." })}
            </p>
          </div>
        </div>
      )}

      {isLoading && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div className="text-center">
            <div className="w-12 h-12 border-4 border-zinc-600 border-t-blue-500 rounded-full animate-spin mx-auto mb-4" />
            <p className="text-zinc-300 text-sm">
              {L({ "zh-CN": "加载文件中...", "en-US": "Loading file..." })}
            </p>
          </div>
        </div>
      )}

      {resourcesLoaded && !schematicInfo && !loadError && (
        <div className="absolute inset-0 z-10 flex items-center justify-center pointer-events-none">
          <div className="pointer-events-auto">
            <div className="bg-black/80 backdrop-blur-md rounded-2xl p-8 border border-zinc-800 shadow-2xl max-w-md w-full mx-4">
              <div className="text-center mb-6">
                <FileBox className="w-16 h-16 mx-auto mb-4 text-zinc-500" />
                <h2 className="text-xl font-bold text-white mb-2">
                  {L({ "zh-CN": "投影预览", "en-US": "Schematic Viewer" })}
                </h2>
                <p className="text-zinc-400 text-sm">
                  {L({
                    "zh-CN": "拖放 .litematic/.schem/.schematic 文件到此处",
                    "en-US": "Drop .litematic/.schem/.schematic file here",
                  })}
                </p>
              </div>

              <Label className="block cursor-pointer">
                <div className="border-2 border-dashed border-zinc-700 hover:border-blue-500 rounded-xl p-6 transition-colors text-center">
                  <Upload className="w-10 h-10 mx-auto mb-2 text-zinc-500" />
                  <p className="text-sm text-zinc-400">
                    {L({
                      "zh-CN": "点击选择文件",
                      "en-US": "Click to select file",
                    })}
                  </p>
                </div>
                <Input
                  ref={fileInputRef}
                  type="file"
                  accept=".litematic,.nbt,.schem,.schematic"
                  className="hidden"
                  onChange={(e) => {
                    const file = e.target.files?.[0];
                    if (file) handleFileLoad(file);
                    if (fileInputRef.current) fileInputRef.current.value = "";
                  }}
                />
              </Label>

              <p className="text-xs text-zinc-600 text-center mt-4">
                Powered by <a href="https://github.com/misode/deepslate" className="text-blue-400 hover:underline" target="_blank" rel="noopener noreferrer">DeepSlate</a>
              </p>
            </div>
          </div>
        </div>
      )}

      {schematicInfo && (
        <>
          <div className="absolute bottom-4 left-4 z-20 flex flex-col gap-2">
            <Button
              variant="secondary"
              size="icon"
              className="h-9 w-9 rounded-full bg-black/60 backdrop-blur border border-zinc-700 hover:bg-black/80"
              onClick={() => setShowMaterialList(!showMaterialList)}
              title={L({ "zh-CN": "材料清单", "en-US": "Material List" })}
            >
              <List className="w-4 h-4" />
            </Button>

            <Button
              variant="secondary"
              size="icon"
              className="h-9 w-9 rounded-full bg-black/60 backdrop-blur border border-zinc-700 hover:bg-black/80"
              onClick={resetView}
              title={L({ "zh-CN": "重置视角", "en-US": "Reset View" })}
            >
              <RotateCcw className="w-4 h-4" />
            </Button>
          </div>

          {showMaterialList && blockCounts && (
            <div className="absolute bottom-4 left-16 z-20 bg-black/80 backdrop-blur-md border border-zinc-700 rounded-xl shadow-2xl max-h-[80vh] overflow-hidden w-72">
              <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-700">
                <h3 className="text-sm font-medium text-white">
                  {L({ "zh-CN": "材料清单", "en-US": "Material List" })}
                </h3>
                <div className="flex gap-1">
                  <Button variant="ghost" size="icon" className="h-6 w-6" onClick={exportCSV}>
                    <Download className="w-3 h-3" />
                  </Button>
                  <Button variant="ghost" size="icon" className="h-6 w-6" onClick={() => setShowMaterialList(false)}>
                    <X className="w-3 h-3" />
                  </Button>
                </div>
              </div>
              <div className="overflow-y-auto max-h-[calc(80vh-48px)]" style={{ columnWidth: "auto" }}>
                {Object.entries(blockCounts)
                  .sort(([, a], [, b]) => b - a)
                  .map(([key, val]) => (
                    <div
                      key={key}
                      className="flex items-center justify-between px-4 py-1.5 text-xs border-b border-zinc-800/50 last:border-0 hover:bg-zinc-800/50"
                    >
                      <span className="truncate mr-2 text-zinc-300">{key.replace("minecraft:", "")}</span>
                      <span className="font-mono text-zinc-500 flex-shrink-0">{val.toLocaleString()}</span>
                    </div>
                  ))}
              </div>
            </div>
          )}

          {maxY > 0 && (
            <div className="absolute top-1/2 -translate-y-1/2 left-2 z-20 flex items-center gap-3" style={{ height: "60vh" }}>
              <div className="flex flex-col items-center gap-1 h-full">
                <input
                  type="range"
                  min={0}
                  max={maxY - 1}
                  value={maxLayer}
                  onChange={(e) => handleLayerChange(minLayer, parseInt(e.target.value))}
                  className="h-full"
                  style={{
                    writingMode: "vertical-lr",
                    direction: "rtl",
                    appearance: "slider-vertical" as any,
                    width: "20px",
                  }}
                  title={L({ "zh-CN": "最大层", "en-US": "Max Layer" })}
                />
                <span className="text-xs text-zinc-500">{maxLayer}</span>
              </div>
              <div className="flex flex-col items-center gap-1 h-full">
                <input
                  type="range"
                  min={0}
                  max={maxY - 1}
                  value={minLayer}
                  onChange={(e) => handleLayerChange(parseInt(e.target.value), maxLayer)}
                  className="h-full"
                  style={{
                    writingMode: "vertical-lr",
                    direction: "rtl",
                    appearance: "slider-vertical" as any,
                    width: "20px",
                  }}
                  title={L({ "zh-CN": "最小层", "en-US": "Min Layer" })}
                />
                <span className="text-xs text-zinc-500">{minLayer}</span>
              </div>
            </div>
          )}

          <div className="absolute top-4 right-4 z-20 flex flex-col gap-2 items-end">
            <div className="flex gap-2">
              <Button
                variant="secondary"
                size="icon"
                className="h-9 w-9 rounded-full bg-blue-600/80 hover:bg-blue-600 backdrop-blur border border-blue-500 text-white"
                onClick={exitToSchematics}
                title={L({ "zh-CN": "退出并返回 Schematics 页面 (Esc)", "en-US": "Back to Schematics (Esc)" })}
              >
                <ArrowLeft className="w-4 h-4" />
              </Button>
              <Button
                variant="secondary"
                size="icon"
                className="h-9 w-9 rounded-full bg-black/60 backdrop-blur border border-zinc-700 hover:bg-black/80"
                onClick={toggleFullscreen}
                title={isFullscreen ? L({ "zh-CN": "退出全屏", "en-US": "Exit Fullscreen" }) : L({ "zh-CN": "全屏显示", "en-US": "Enter Fullscreen" })}
              >
                {isFullscreen ? <Minimize2 className="w-4 h-4" /> : <Maximize2 className="w-4 h-4" />}
              </Button>
            </div>

            <Button
              variant="secondary"
              size="icon"
              className="h-9 w-9 rounded-full bg-black/60 backdrop-blur border border-zinc-700 hover:bg-black/80"
              onClick={() => setShowSettings(!showSettings)}
              title={L({ "zh-CN": "设置", "en-US": "Settings" })}
            >
              <Settings className="w-4 h-4" />
            </Button>

            <Button
              variant="secondary"
              size="sm"
              className="bg-black/60 backdrop-blur border border-zinc-700 hover:bg-black/80 text-xs h-8"
              onClick={unloadSchematic}
            >
              {L({ "zh-CN": "卸载投影", "en-US": "Unload Schematic" })}
            </Button>
          </div>

          <div
            className="absolute top-0 right-0 z-30 h-full bg-black/95 backdrop-blur-md border-l border-zinc-800 shadow-2xl overflow-y-auto transition-all duration-300"
            style={{ width: showSettings ? "380px" : "0" }}
          >
            <div className="p-6 min-w-[380px]">
              <div className="flex items-center justify-between mb-6">
                <h2 className="text-lg font-bold text-white">
                  {L({ "zh-CN": "设置", "en-US": "Settings" })}
                </h2>
                <Button variant="ghost" size="icon" className="h-8 w-8" onClick={() => setShowSettings(false)}>
                  <X className="w-4 h-4" />
                </Button>
              </div>

              {schematicInfo && (
                <div className="mb-6 space-y-3 text-sm">
                  <h3 className="text-sm font-medium text-zinc-400 flex items-center gap-2">
                    <Info className="w-4 h-4" />
                    {L({ "zh-CN": "投影信息", "en-US": "Schematic Info" })}
                  </h3>
                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div>
                      <div className="text-zinc-500">{L({ "zh-CN": "名称", "en-US": "Name" })}</div>
                      <div className="text-white truncate">{schematicInfo.name}</div>
                    </div>
                    <div>
                      <div className="text-zinc-500">{L({ "zh-CN": "作者", "en-US": "Author" })}</div>
                      <div className="text-white truncate">{schematicInfo.author}</div>
                    </div>
                    <div>
                      <div className="text-zinc-500">{L({ "zh-CN": "尺寸", "en-US": "Size" })}</div>
                      <div className="text-white font-mono">{schematicInfo.size}</div>
                    </div>
                    <div>
                      <div className="text-zinc-500">{L({ "zh-CN": "方块总数", "en-US": "Blocks" })}</div>
                      <div className="text-white font-mono">{schematicInfo.blocks.toLocaleString()}</div>
                    </div>
                    <div className="col-span-2">
                      <div className="text-zinc-500">{L({ "zh-CN": "创建时间", "en-US": "Created" })}</div>
                      <div className="text-white truncate">{schematicInfo.date}</div>
                    </div>
                    {schematicInfo.description && (
                      <div className="col-span-2">
                        <div className="text-zinc-500">{L({ "zh-CN": "描述", "en-US": "Description" })}</div>
                        <div className="text-white">{schematicInfo.description}</div>
                      </div>
                    )}
                  </div>
                </div>
              )}

              <div className="mb-6">
                <h3 className="text-sm font-medium text-zinc-400 mb-3">
                  {L({ "zh-CN": "操作", "en-US": "Controls" })}
                </h3>
                <div className="space-y-4">
                  <div>
                    <Label className="text-xs text-zinc-500 mb-2 block">
                      {L({ "zh-CN": "左键拖拽", "en-US": "Click-drag" })}
                    </Label>
                    <div className="flex gap-2 items-center">
                      <select
                        className="bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-xs"
                        defaultValue={typeof window !== "undefined" ? localStorage.getItem("click-drag") || "move" : "move"}
                        onChange={(e) => {
                          try { localStorage.setItem("click-drag", e.target.value); } catch {}
                        }}
                      >
                        <option value="move">{L({ "zh-CN": "移动", "en-US": "Move" })}</option>
                        <option value="pan">{L({ "zh-CN": "旋转", "en-US": "Pan" })}</option>
                      </select>
                      <label className="flex items-center gap-1 text-xs text-zinc-400">
                        <input
                          type="checkbox"
                          defaultChecked={typeof window !== "undefined" ? localStorage.getItem("click-drag-invert") === "true" : false}
                          onChange={(e) => {
                            try { localStorage.setItem("click-drag-invert", String(e.target.checked)); } catch {}
                          }}
                        />
                        {L({ "zh-CN": "反转", "en-US": "Invert" })}
                      </label>
                    </div>
                  </div>
                  <div>
                    <Label className="text-xs text-zinc-500 mb-2 block">
                      {L({ "zh-CN": "中键拖拽", "en-US": "Middle-click-drag" })}
                    </Label>
                    <div className="flex gap-2 items-center">
                      <select
                        className="bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-xs"
                        defaultValue={typeof window !== "undefined" ? localStorage.getItem("middle-click-drag") || "pan" : "pan"}
                        onChange={(e) => {
                          try { localStorage.setItem("middle-click-drag", e.target.value); } catch {}
                        }}
                      >
                        <option value="move">{L({ "zh-CN": "移动", "en-US": "Move" })}</option>
                        <option value="pan">{L({ "zh-CN": "旋转", "en-US": "Pan" })}</option>
                      </select>
                      <label className="flex items-center gap-1 text-xs text-zinc-400">
                        <input
                          type="checkbox"
                          defaultChecked={typeof window !== "undefined" ? localStorage.getItem("middle-click-drag-invert") === "true" : false}
                          onChange={(e) => {
                            try { localStorage.setItem("middle-click-drag-invert", String(e.target.checked)); } catch {}
                          }}
                        />
                        {L({ "zh-CN": "反转", "en-US": "Invert" })}
                      </label>
                    </div>
                  </div>
                </div>
              </div>

              <div>
                <h3 className="text-sm font-medium text-zinc-400 mb-3">
                  {L({ "zh-CN": "快捷键", "en-US": "Shortcuts" })}
                </h3>
                <div className="text-xs text-zinc-500 space-y-1">
                  <p>
                    WASD / Arrow Keys: {L({ "zh-CN": "移动", "en-US": "Move" })}
                  </p>
                  <p>
                    Shift / Space: {L({ "zh-CN": "上下", "en-US": "Up/Down" })}
                  </p>
                  <p>
                    Scroll: {L({ "zh-CN": "缩放", "en-US": "Zoom" })}
                  </p>
                  <p>
                    {L({ "zh-CN": "触屏: 单指旋转, 双指平移+缩放", "en-US": "Touch: 1-finger pan, 2-finger move+zoom" })}
                  </p>
                </div>
              </div>

              <div className="mt-6 pt-4 border-t border-zinc-800">
                <p className="text-xs text-zinc-600">
                  Powered by <a href="https://github.com/misode/deepslate" className="text-blue-400 hover:underline" target="_blank" rel="noopener noreferrer">DeepSlate</a>
                </p>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}