"use client";

import { useRef, useState, useEffect } from "react";
import { useTheme } from "next-themes";
import {
  useSettings,
  COLOR_PRESETS,
  FONT_SIZE_MIN,
  FONT_SIZE_MAX,
  BG_BLUR_MIN,
  BG_BLUR_MAX,
  BG_OPACITY_MIN,
  BG_OPACITY_MAX,
  type ThemeMode,
} from "@/components/settings/settings-provider";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { ImagePlus, RotateCcw, Sparkles, Sun, Moon, Type, X } from "lucide-react";
import { cn } from "@/lib/utils";

// ============================================================
// 主题模式（浅色 / 深色）
// ============================================================
function ThemeModeRow({
  value,
  onChange,
}: {
  value: ThemeMode;
  onChange: (v: ThemeMode) => void;
}) {
  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <Label className="font-medium text-sm">主题模式</Label>
        <span className="text-xs text-muted-foreground">{value === "light" ? "浅色" : "深色"}</span>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Button
          type="button"
          variant={value === "light" ? "default" : "outline"}
          onClick={() => onChange("light")}
          className="gap-2 h-8"
        >
          <Sun className="size-3.5" />
          浅色
        </Button>
        <Button
          type="button"
          variant={value === "dark" ? "default" : "outline"}
          onClick={() => onChange("dark")}
          className="gap-2 h-8"
        >
          <Moon className="size-3.5" />
          深色
        </Button>
      </div>
    </div>
  );
}

// ============================================================
// 主题色：简约自定义调色盘
// ============================================================
function ThemeColorRow({
  value,
  onChange,
}: {
  value: string; // "default" 或 oklch 字符串
  onChange: (v: string) => void;
}) {
  const currentColor = value === "default" ? "#1f1f1f" : oklchToHex(value) ?? "#1f1f1f";

  const onCustomColor = (hex: string) => {
    const oklch = hexToOklch(hex);
    if (oklch) onChange(oklch);
  };

  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <Label className="font-medium text-sm">主题色</Label>
      </div>

      <div className="flex items-center gap-3 rounded-lg border border-border bg-card p-2.5">
        {/* 简约拾色器：一个方色块 */}
        <label className="relative shrink-0 cursor-pointer">
          <input
            type="color"
            value={currentColor}
            onChange={(e) => onCustomColor(e.target.value)}
            className="h-8 w-8 cursor-pointer appearance-none rounded-md border border-border bg-transparent p-0"
          />
        </label>

        {/* 水平色条：直观展示主色 */}
        <div
          className="h-2 flex-1 rounded-full"
          style={{ backgroundColor: currentColor }}
        />

        {/* 当前 HEX */}
        <span className="shrink-0 font-mono text-xs text-muted-foreground">
          {currentColor.toUpperCase()}
        </span>

        {/* 重置 */}
        {value !== "default" && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onChange("default")}
            className="h-7 px-2 text-xs shrink-0"
          >
            重置
          </Button>
        )}
      </div>
    </div>
  );
}

// ============================================================
// 字体大小：滑块（1~30 px，整数）
// ============================================================
function FontSizeRow({
  value,
  onChange,
}: {
  value: number;
  onChange: (v: number) => void;
}) {
  const rounded = Math.round(value);

  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <Label className="font-medium text-sm flex items-center gap-1.5">
          <Type className="size-3.5 text-muted-foreground" />
          字体大小
        </Label>
        <span className="text-xs text-muted-foreground font-mono">{rounded}px</span>
      </div>

      <div className="flex items-center gap-3">
        <input
          type="range"
          min={FONT_SIZE_MIN}
          max={FONT_SIZE_MAX}
          step={1}
          value={rounded}
          onChange={(e) => onChange(parseInt(e.target.value, 10))}
          className="accent-primary h-1.5 flex-1 cursor-pointer appearance-none rounded-full bg-muted"
        />
      </div>
    </div>
  );
}

// ============================================================
// 背景图片：上传 + 不透明度 + 模糊（高斯）
// ============================================================
function BackgroundRow({
  value,
  onChange,
}: {
  value: { imageDataUrl?: string; opacity: number; blur: number };
  onChange: (v: { imageDataUrl?: string; opacity: number; blur: number }) => void;
}) {
  const fileRef = useRef<HTMLInputElement>(null);
  const hasImage = !!value.imageDataUrl;

  const handleFile = (f: File) => {
    const reader = new FileReader();
    reader.onload = () => {
      onChange({ ...value, imageDataUrl: reader.result as string });
    };
    reader.readAsDataURL(f);
  };

  const opacityPct = Math.round(value.opacity * 100);
  const blurInt = Math.round(value.blur);

  return (
    <div className="space-y-2.5">
      <div className="flex items-center justify-between">
        <Label className="font-medium text-sm">页面背景</Label>
        {hasImage && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onChange({ ...value, imageDataUrl: undefined })}
            className="h-7 px-2 text-xs gap-1"
          >
            <X className="size-3" />
            移除
          </Button>
        )}
      </div>

      {/* 上传区 / 图片预览 —— 更紧凑的高度 */}
      <div
        className={cn(
          "relative flex items-center justify-center rounded-lg border border-dashed overflow-hidden transition-colors",
          hasImage ? "border-border h-28" : "border-border hover:border-primary/50 h-24"
        )}
      >
        {hasImage ? (
          <>
            <div
              className="absolute inset-0"
              style={{
                backgroundImage: `url(${value.imageDataUrl})`,
                backgroundSize: "cover",
                backgroundPosition: "center",
                filter: `blur(${value.blur}px)`,
                opacity: value.opacity,
                transform: "scale(1.08)",
              }}
            />
            <div className="relative z-10 rounded-full bg-black/40 px-3 py-1 text-xs text-white backdrop-blur">
              预览
            </div>
          </>
        ) : (
          <button
            type="button"
            onClick={() => fileRef.current?.click()}
            className="flex w-full flex-col items-center gap-1.5 py-3 text-muted-foreground transition-colors hover:text-foreground"
          >
            <ImagePlus className="size-6" />
            <span className="text-xs">上传背景图片</span>
          </button>
        )}
        <input
          ref={fileRef}
          type="file"
          accept="image/*"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) handleFile(f);
            e.target.value = "";
          }}
        />
      </div>

      {/* 不透明度滑块 */}
      <div className="space-y-1.5">
        <div className="flex items-center justify-between">
          <Label className="text-xs text-muted-foreground">不透明度</Label>
          <span className="text-xs text-muted-foreground font-mono">{opacityPct}%</span>
        </div>
        <input
          type="range"
          min={BG_OPACITY_MIN}
          max={BG_OPACITY_MAX}
          step={1}
          value={opacityPct}
          disabled={!hasImage}
          onChange={(e) => onChange({ ...value, opacity: parseInt(e.target.value, 10) / 100 })}
          className="accent-primary h-1.5 w-full cursor-pointer appearance-none rounded-full bg-muted disabled:cursor-not-allowed"
        />
      </div>

      {/* 高斯模糊滑块 */}
      <div className="space-y-1.5">
        <div className="flex items-center justify-between">
          <Label className="text-xs text-muted-foreground">高斯模糊</Label>
          <span className="text-xs text-muted-foreground font-mono">{blurInt}px</span>
        </div>
        <input
          type="range"
          min={BG_BLUR_MIN}
          max={BG_BLUR_MAX}
          step={1}
          value={blurInt}
          disabled={!hasImage}
          onChange={(e) => onChange({ ...value, blur: parseInt(e.target.value, 10) })}
          className="accent-primary h-1.5 w-full cursor-pointer appearance-none rounded-full bg-muted disabled:cursor-not-allowed"
        />
      </div>
    </div>
  );
}

// ============================================================
// 主组件
// ============================================================
export function AppearanceSection() {
  const { settings, update, reset } = useSettings();
  const { appearance } = settings;
  const { setTheme: setNextTheme } = useTheme();
  const [hydrated, setHydrated] = useState(false);

  useEffect(() => setHydrated(true), []);

  // 主题模式切换时同步到 next-themes，确保全站一致
  const handleThemeModeChange = (mode: ThemeMode) => {
    update("appearance", { themeMode: mode });
    setNextTheme(mode);
  };

  if (!hydrated) return null;

  return (
    <Card id="section-appearance" className="scroll-mt-4">
      <CardHeader className="pb-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <CardTitle className="flex items-center gap-2 text-base">
              <Sparkles className="size-4 text-primary" />
              外观
            </CardTitle>
            <CardDescription className="text-xs mt-1">主题、配色、字体大小与页面背景</CardDescription>
          </div>
          <Button type="button" variant="outline" size="sm" onClick={reset} className="shrink-0 gap-1.5 h-8">
            <RotateCcw className="size-3.5" />
            重置
          </Button>
        </div>
      </CardHeader>

      <CardContent className="space-y-4">
        <ThemeModeRow
          value={appearance.themeMode}
          onChange={handleThemeModeChange}
        />

        <div className="h-px bg-border" />

        <ThemeColorRow
          value={appearance.themeColor}
          onChange={(v) => update("appearance", { themeColor: v })}
        />

        <div className="h-px bg-border" />

        <FontSizeRow
          value={appearance.fontSize}
          onChange={(v) => update("appearance", { fontSize: v })}
        />

        <div className="h-px bg-border" />

        <BackgroundRow
          value={appearance.background}
          onChange={(v) => update("appearance", { background: v })}
        />
      </CardContent>
    </Card>
  );
}

// ============================================================
// 颜色转换工具：oklch <-> hex
// （简化实现，满足主题色 picker 的显示需求）
// ============================================================
function oklchToHex(oklchStr: string): string | null {
  const m = oklchStr.match(/oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)/);
  if (!m) return null;
  const l = parseFloat(m[1]); // 0 ~ 1
  const c = parseFloat(m[2]); // 0 ~ ~0.37
  const h = parseFloat(m[3]); // 0 ~ 360

  // OKLCH -> OKLAB
  const hRad = (h * Math.PI) / 180;
  const a = c * Math.cos(hRad);
  const b = c * Math.sin(hRad);

  // OKLAB -> Linear sRGB (approximate)
  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.291485548 * b;

  const L = l_ * l_ * l_;
  const M = m_ * m_ * m_;
  const S = s_ * s_ * s_;

  const rLinear = +4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S;
  const gLinear = -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S;
  const bLinear = -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S;

  // Linear sRGB -> sRGB (gamma)
  const toSrgb = (v: number) => {
    const abs = Math.abs(v);
    if (abs <= 0.0031308) return v * 12.92;
    return 1.055 * Math.sign(v) * Math.pow(abs, 1 / 2.4) - 0.055;
  };

  const r = Math.max(0, Math.min(1, toSrgb(rLinear)));
  const g = Math.max(0, Math.min(1, toSrgb(gLinear)));
  const bCh = Math.max(0, Math.min(1, toSrgb(bLinear)));

  const toHex = (v: number) =>
    Math.round(v * 255)
      .toString(16)
      .padStart(2, "0");

  return `#${toHex(r)}${toHex(g)}${toHex(bCh)}`;
}

function hexToOklch(hex: string): string | null {
  const m = hex.match(/^#?([\da-f]{6})$/i);
  if (!m) return null;
  const r = parseInt(m[1].slice(0, 2), 16) / 255;
  const g = parseInt(m[1].slice(2, 4), 16) / 255;
  const b = parseInt(m[1].slice(4, 6), 16) / 255;

  // sRGB -> Linear sRGB
  const toLin = (v: number) => (v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4));
  const rLin = toLin(r);
  const gLin = toLin(g);
  const bLin = toLin(b);

  // Linear sRGB -> OKLAB
  const L = 0.4122214708 * rLin + 0.5363325363 * gLin + 0.0514459929 * bLin;
  const M = 0.2119034982 * rLin + 0.6806995451 * gLin + 0.1073969566 * bLin;
  const S = 0.0883024619 * rLin + 0.2817188376 * gLin + 0.6299787005 * bLin;

  const l_ = Math.cbrt(L);
  const m_ = Math.cbrt(M);
  const s_ = Math.cbrt(S);

  const l = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
  const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
  const b2 = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;

  // OKLAB -> OKLCH
  const c = Math.sqrt(a * a + b2 * b2);
  let hDeg = (Math.atan2(b2, a) * 180) / Math.PI;
  if (hDeg < 0) hDeg += 360;
  if (c < 0.0001) hDeg = 0;

  return `oklch(${l.toFixed(4)} ${c.toFixed(4)} ${hDeg.toFixed(2)})`;
}