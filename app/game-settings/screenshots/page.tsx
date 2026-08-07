"use client";

import { useState, useEffect } from "react";
import { Camera, Copy, Check, X, Plus } from "lucide-react";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { blobToBase64 } from "@/lib/file-utils";

interface ScreenshotFile {
  name: string;
  path: string;
  size: number;
}

export default function GameSettingsScreenshots() {
  const { config, configLoaded } = useLaunchContext();
  const [screenshots, setScreenshots] = useState<ScreenshotFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedImage, setSelectedImage] = useState<{ path: string; base64: string; name: string } | null>(null);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [imageCache, setImageCache] = useState<Record<string, string>>({});

  const effectiveVersionDirName = config.loadName || config.versionName;
  const screenshotsDir = config.minecraftPath && effectiveVersionDirName
    ? `${config.minecraftPath}/versions/${effectiveVersionDirName}/screenshots`
    : undefined;

  useEffect(() => {
    if (!configLoaded || !screenshotsDir) {
      const timer = window.setTimeout(() => setLoading(false), 0);
      return () => window.clearTimeout(timer);
    }

    const loadScreenshots = async () => {
      try {
        setLoading(true);
        setError(null);
        
        const files = await invoke<Array<{ name: string; path: string; size: number; is_dir: boolean }>>(
          "vm_list_dir", 
          { dirPath: screenshotsDir, extensionsFilter: ["png", "jpg", "jpeg", "gif", "bmp"] }
        );
        
        const imageFiles = files
          .filter(f => !f.is_dir)
          .sort((a, b) => b.name.localeCompare(a.name));
        
        setScreenshots(imageFiles);
      } catch (err) {
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    loadScreenshots();
  }, [configLoaded, screenshotsDir]);

  const getImageBase64 = async (path: string): Promise<string> => {
    if (imageCache[path]) return imageCache[path];
    
    const base64 = await invoke<string>("read_file_base64", { path });
    setImageCache(prev => ({ ...prev, [path]: base64 }));
    return base64;
  };

  const handleCopyToClipboard = async (path: string, index: number) => {
    try {
      const base64 = await getImageBase64(path);
      const blob = await fetch(`data:image/png;base64,${base64}`).then(r => r.blob());
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      setCopiedIndex(index);
      setTimeout(() => setCopiedIndex(null), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const handleUploadFiles = async () => {
    if (!screenshotsDir) return;
    
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.accept = ".png,.jpg,.jpeg,.gif,.bmp";
    input.style.display = 'none';
    
    document.body.appendChild(input);
    
    input.addEventListener('change', async (e) => {
      const files = (e.target as HTMLInputElement).files;
      if (!files || files.length === 0) {
        document.body.removeChild(input);
        return;
      }

      for (const file of Array.from(files)) {
        const base64 = await blobToBase64(file);
        await invoke("vm_write_file_base64", {
          dirPath: screenshotsDir,
          fileName: file.name,
          contentBase64: base64,
        });
      }
      
      // 刷新截图列表
      const refreshedFiles = await invoke<Array<{ name: string; path: string; size: number; is_dir: boolean }>>(
        "vm_list_dir", 
        { dirPath: screenshotsDir, extensionsFilter: ["png", "jpg", "jpeg", "gif", "bmp"] }
      );
      
      const imageFiles = refreshedFiles
        .filter(f => !f.is_dir)
        .sort((a, b) => b.name.localeCompare(a.name));
      
      setScreenshots(imageFiles);
      setImageCache({});
      
      document.body.removeChild(input);
    });
    
    input.click();
  };

  const formatSize = (bytes: number) => {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
    return (bytes / (1024 * 1024)).toFixed(2) + " MB";
  };

  if (!configLoaded) {
    return (
      <div className="h-full flex items-center justify-center">
        <p className="text-sm text-muted-foreground">加载中...</p>
      </div>
    );
  }

  if (!config.minecraftPath) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-3 text-center p-4">
        <Camera className="size-12 text-muted-foreground" />
        <p className="text-sm font-medium">未配置游戏目录</p>
        <p className="text-xs text-muted-foreground">请先配置游戏目录路径</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <div className="flex items-center gap-3">
          <div className="flex size-9 items-center justify-center rounded-xl bg-pink-500/10">
            <Camera className="size-5 text-pink-500" />
          </div>
          <div>
            <h1 className="text-base font-semibold">截图管理</h1>
            <p className="text-xs text-muted-foreground">{screenshots.length} 张截图</p>
          </div>
        </div>
        <Button variant="default" size="icon" className="size-8" onClick={handleUploadFiles} title="上传截图">
          <Plus className="size-3.5" />
        </Button>
      </div>

      <div className="flex-1 overflow-auto p-4">
        {loading ? (
          <div className="flex h-full items-center justify-center">
            <div className="size-8 border-2 border-border border-t-foreground rounded-full animate-spin" />
          </div>
        ) : error ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <p className="text-sm font-medium">加载截图失败</p>
            <p className="text-xs text-muted-foreground">{error}</p>
          </div>
        ) : screenshots.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
            <Camera className="size-12 text-muted-foreground" />
            <p className="text-sm font-medium">暂无截图</p>
            <p className="text-xs text-muted-foreground">游戏内按 F2 键截图后会显示在这里</p>
          </div>
        ) : (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
            {screenshots.map((screenshot, index) => (
              <ScreenshotCard
                key={screenshot.path}
                screenshot={screenshot}
                index={index}
                copiedIndex={copiedIndex}
                onCopy={handleCopyToClipboard}
                onPreview={async () => {
                  const base64 = await getImageBase64(screenshot.path);
                  setSelectedImage({ path: screenshot.path, base64, name: screenshot.name });
                }}
                formatSize={formatSize}
              />
            ))}
          </div>
        )}
      </div>

      {selectedImage && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4" onClick={() => setSelectedImage(null)}>
          <div className="relative max-w-full max-h-full" onClick={(e) => e.stopPropagation()}>
            <img
              src={`data:image/png;base64,${selectedImage.base64}`}
              alt={selectedImage.name}
              className="max-w-full max-h-[85vh] object-contain rounded-lg"
            />
            <div className="absolute top-2 right-2 flex gap-2">
              <Button variant="secondary" size="icon" className="bg-white/20 hover:bg-white/30" onClick={() => handleCopyToClipboard(selectedImage.path, -1)}>
                <Copy className="size-4" />
              </Button>
              <Button variant="secondary" size="icon" className="bg-white/20 hover:bg-white/30" onClick={() => setSelectedImage(null)}>
                <X className="size-4" />
              </Button>
            </div>
            <div className="absolute bottom-2 left-2 right-2 bg-black/60 rounded-lg px-3 py-2">
              <p className="text-sm text-white truncate">{selectedImage.name}</p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface ScreenshotCardProps {
  screenshot: ScreenshotFile;
  index: number;
  copiedIndex: number | null;
  onCopy: (path: string, index: number) => void;
  onPreview: () => void;
  formatSize: (bytes: number) => string;
}

function ScreenshotCard({ screenshot, index, copiedIndex, onCopy, onPreview, formatSize }: ScreenshotCardProps) {
  const [base64, setBase64] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadImage = async () => {
      try {
        const result = await invoke<string>("read_file_base64", { path: screenshot.path });
        setBase64(result);
      } catch (err) {
        console.error("Failed to load image:", err);
      } finally {
        setIsLoading(false);
      }
    };
    loadImage();
  }, [screenshot.path]);

  return (
    <div
      className="relative aspect-square rounded-xl overflow-hidden bg-muted cursor-pointer hover:ring-2 hover:ring-foreground/50 transition-all group"
      onClick={onPreview}
    >
      {isLoading ? (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="size-6 border-2 border-border border-t-foreground rounded-full animate-spin" />
        </div>
      ) : base64 ? (
        <img src={`data:image/png;base64,${base64}`} alt={screenshot.name} className="w-full h-full object-cover" />
      ) : (
        <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
          <Camera className="size-8" />
        </div>
      )}

      <div className="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center">
        <Button
          variant="secondary"
          size="icon"
          className="size-8 rounded-full"
          onClick={(e) => { e.stopPropagation(); onCopy(screenshot.path, index); }}
        >
          {copiedIndex === index ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
        </Button>
      </div>

      <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/80 to-transparent p-2">
        <p className="text-xs text-white truncate">{screenshot.name}</p>
        <p className="text-[10px] text-white/70">{formatSize(screenshot.size)}</p>
      </div>
    </div>
  );
}
