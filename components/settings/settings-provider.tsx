"use client";

import * as React from "react";

// ============================================================
// 类型定义
// ============================================================
export type ThemeMode = "light" | "dark";
export type HomeMode = "simple" | "full";
export type AppLanguage = "zh-CN" | "en-US";

export interface BackgroundConfig {
  imageDataUrl?: string;
  opacity: number; // 0 ~ 1（精确到 0.01）
  blur: number; // 0 ~ 20（整数 px）
}

// 自定义主题色：
//   "default" = 使用 shadcn 原主题色
//   其他字符串 = 用户选择的 oklch 色值（如 "oklch(0.6 0.2 155)"）
//   会自动派生整站配色方案
export type ThemeColor = "default" | string;

export interface AppearanceSettings {
  themeMode: ThemeMode;
  themeColor: ThemeColor; // "default" 或 oklch 字符串
  fontSize: number; // 12 ~ 18（整数 px，仅影响文字）
  background: BackgroundConfig;
  homeMode: HomeMode; // 主页模式：simple 或 full
}

export interface GeneralSettings {
  language: AppLanguage;
  /** 下载 Modrinth 或 CurseForge 模组时自动下载其必需依赖。 */
  autoDownloadModDependencies: boolean;
}

export interface LauncherSettings {
  general: GeneralSettings;
  appearance: AppearanceSettings;
}

// ============================================================
// 预设自定义主题色（调色盘快选按钮）
//   颜色使用 oklch(l c h) 格式 —— 通过改变 l/c 自动派生整站
// ============================================================
export interface ColorPreset {
  name: string;
  oklch: string; // 主色的 oklch 值
}

export const COLOR_PRESETS: ColorPreset[] = [
  { name: "深色", oklch: "oklch(0.216 0.006 56.043)" }, // shadcn 默认深色 primary
  { name: "翡翠", oklch: "oklch(0.6 0.16 155)" },
  { name: "天蓝", oklch: "oklch(0.58 0.18 240)" },
  { name: "玫瑰", oklch: "oklch(0.58 0.22 10)" },
  { name: "琥珀", oklch: "oklch(0.65 0.18 75)" },
  { name: "紫罗兰", oklch: "oklch(0.58 0.2 300)" },
  { name: "青绿", oklch: "oklch(0.55 0.15 185)" },
  { name: "赤红", oklch: "oklch(0.55 0.22 25)" },
];

// ============================================================
// 默认值
// ============================================================
export const DEFAULT_SETTINGS: LauncherSettings = {
  general: {
    language: "zh-CN",
    autoDownloadModDependencies: true,
  },
  appearance: {
    themeMode: "light",
    themeColor: "default",
    fontSize: 14,
    background: {
      opacity: 0.55,
      blur: 6,
    },
    homeMode: "full", // 默认完整模式
  },
};

// 字体大小范围（整数）
export const FONT_SIZE_MIN = 1;
export const FONT_SIZE_MAX = 30;

// 背景模糊范围（整数 px）
export const BG_BLUR_MIN = 0;
export const BG_BLUR_MAX = 20;

// 背景不透明度的百分比表示：0% ~ 100%
export const BG_OPACITY_MIN = 0;
export const BG_OPACITY_MAX = 100;

const STORAGE_KEY = "rtlauncher:settings:v3";

function isAppLanguage(value: unknown): value is AppLanguage {
  return value === "zh-CN" || value === "en-US";
}

export function languageFromSystemPreference(preferred: string | undefined): AppLanguage {
  if (!preferred) return "zh-CN";
  return preferred.toLowerCase().startsWith("zh") ? "zh-CN" : "en-US";
}

/** 首次使用时根据系统（WebView）首选语言决定默认界面语言。 */
export function detectSystemLanguage(): AppLanguage {
  if (typeof navigator === "undefined") return "zh-CN";
  return languageFromSystemPreference(navigator.languages?.[0] || navigator.language);
}

// ============================================================
// Provider
// ============================================================
interface SettingsContextValue {
  settings: LauncherSettings;
  update: <K extends keyof LauncherSettings>(
    section: K,
    patch: Partial<LauncherSettings[K]>
  ) => void;
  reset: () => void;
}

const SettingsContext = React.createContext<SettingsContextValue | null>(null);

// ============================================================
// oklch 颜色工具 —— 从单个主色自动生成整站配色
// ============================================================
function parseOklch(value: string): { l: number; c: number; h: number } | null {
  // 解析 "oklch(0.6 0.2 155)" 或 "oklch(0.6 0.2 155 / 1)"
  const m = value.match(/oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)/);
  if (!m) return null;
  return { l: parseFloat(m[1]), c: parseFloat(m[2]), h: parseFloat(m[3]) };
}

function makeOklch(l: number, c: number, h: number, alpha?: number): string {
  if (alpha !== undefined && alpha < 1) {
    return `oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(3)} / ${alpha.toFixed(2)})`;
  }
  return `oklch(${l.toFixed(3)} ${c.toFixed(3)} ${h.toFixed(3)})`;
}

// 根据给定的主色 oklch 生成整站配色方案
function derivePalette(primaryOklch: string, isDark: boolean) {
  const primary = parseOklch(primaryOklch);
  if (!primary) {
    // 解析失败，回退到默认 shadcn 配色
    return isDark ? DEFAULT_DARK : DEFAULT_LIGHT;
  }

  const { l, c, h } = primary;

  if (!isDark) {
    // 浅色模式 —— 背景/卡片也带有主色的氛围（保持 h，用极低 c）
    const backgroundC = Math.min(0.008, c * 0.04); // 极低饱和度
    const cardC = Math.min(0.005, c * 0.025);
    const surfaceC = Math.min(0.012, c * 0.06);
    const borderC = Math.min(0.02, c * 0.1);
    const foregroundC = Math.min(0.01, c * 0.05);
    const mutedC = Math.min(0.018, c * 0.09);
    const accentC = Math.min(0.05, c * 0.22);

    return {
      background: makeOklch(0.995, backgroundC, h),
      foreground: makeOklch(0.15, foregroundC, h),
      card: makeOklch(0.995, cardC, h),
      cardForeground: makeOklch(0.15, foregroundC, h),
      popover: makeOklch(0.995, cardC, h),
      popoverForeground: makeOklch(0.15, foregroundC, h),
      primary: primaryOklch,
      primaryForeground: l < 0.55 ? makeOklch(0.99, 0.005, h) : makeOklch(0.15, foregroundC, h),
      secondary: makeOklch(0.96, surfaceC, h),
      secondaryForeground: makeOklch(0.25, Math.min(0.1, c * 0.45), h),
      muted: makeOklch(0.955, surfaceC, h),
      mutedForeground: makeOklch(0.5, mutedC, h),
      accent: makeOklch(0.95, accentC, h),
      accentForeground: makeOklch(0.25, Math.min(0.12, c * 0.5), h),
      border: makeOklch(0.91, borderC, h),
      input: makeOklch(0.91, borderC, h),
      ring: makeOklch(l + 0.1, c, h),
    };
  } else {
    // 深色模式
    return {
      background: makeOklch(0.15, Math.min(0.03, c * 0.15), h),
      foreground: makeOklch(0.98, 0.005, h),
      card: makeOklch(0.22, Math.min(0.04, c * 0.2), h),
      cardForeground: makeOklch(0.98, 0.005, h),
      popover: makeOklch(0.22, Math.min(0.04, c * 0.2), h),
      popoverForeground: makeOklch(0.98, 0.005, h),
      primary: makeOklch(Math.min(0.92, l + 0.2), c, h),
      primaryForeground: makeOklch(0.22, Math.min(0.04, c * 0.2), h),
      secondary: makeOklch(0.28, Math.min(0.05, c * 0.25), h),
      secondaryForeground: makeOklch(0.92, c * 0.2, h),
      muted: makeOklch(0.3, Math.min(0.04, c * 0.2), h),
      mutedForeground: makeOklch(0.75, Math.min(0.08, c * 0.4), h),
      accent: makeOklch(0.26, Math.min(0.05, c * 0.25), h),
      accentForeground: makeOklch(0.9, Math.min(0.08, c * 0.4), h),
      border: makeOklch(1, 0, 0, 0.12),
      input: makeOklch(1, 0, 0, 0.18),
      ring: makeOklch(Math.min(0.78, l + 0.15), c, h),
    };
  }
}

// 默认 shadcn 主题色
const DEFAULT_LIGHT = {
  background: "oklch(1 0 0)",
  foreground: "oklch(0.147 0.004 49.25)",
  card: "oklch(1 0 0)",
  cardForeground: "oklch(0.147 0.004 49.25)",
  popover: "oklch(1 0 0)",
  popoverForeground: "oklch(0.147 0.004 49.25)",
  primary: "oklch(0.216 0.006 56.043)",
  primaryForeground: "oklch(0.985 0.001 106.423)",
  secondary: "oklch(0.97 0.001 106.424)",
  secondaryForeground: "oklch(0.216 0.006 56.043)",
  muted: "oklch(0.97 0.001 106.424)",
  mutedForeground: "oklch(0.553 0.013 58.071)",
  accent: "oklch(0.97 0.001 106.424)",
  accentForeground: "oklch(0.216 0.006 56.043)",
  border: "oklch(0.923 0.003 48.717)",
  input: "oklch(0.923 0.003 48.717)",
  ring: "oklch(0.709 0.01 56.259)",
};

const DEFAULT_DARK = {
  background: "oklch(0.147 0.004 49.25)",
  foreground: "oklch(0.985 0.001 106.423)",
  card: "oklch(0.216 0.006 56.043)",
  cardForeground: "oklch(0.985 0.001 106.423)",
  popover: "oklch(0.216 0.006 56.043)",
  popoverForeground: "oklch(0.985 0.001 106.423)",
  primary: "oklch(0.923 0.003 48.717)",
  primaryForeground: "oklch(0.216 0.006 56.043)",
  secondary: "oklch(0.268 0.007 34.298)",
  secondaryForeground: "oklch(0.985 0.001 106.423)",
  muted: "oklch(0.268 0.007 34.298)",
  mutedForeground: "oklch(0.709 0.01 56.259)",
  accent: "oklch(0.268 0.007 34.298)",
  accentForeground: "oklch(0.985 0.001 106.423)",
  border: "oklch(1 0 0 / 10%)",
  input: "oklch(1 0 0 / 15%)",
  ring: "oklch(0.553 0.013 58.071)",
};

// ============================================================
// DOM 应用函数
// ============================================================

// 应用主题色到 CSS 变量
function applyThemeColorToDom(themeColor: ThemeColor, effectiveIsDark: boolean) {
  const root = document.documentElement;

  let colors;
  if (themeColor === "default") {
    colors = effectiveIsDark ? DEFAULT_DARK : DEFAULT_LIGHT;
  } else {
    colors = derivePalette(themeColor, effectiveIsDark);
  }

  // 页面与容器
  root.style.setProperty("--background", colors.background);
  root.style.setProperty("--foreground", colors.foreground);
  root.style.setProperty("--card", colors.card);
  root.style.setProperty("--card-foreground", colors.cardForeground);
  root.style.setProperty("--popover", colors.popover);
  root.style.setProperty("--popover-foreground", colors.popoverForeground);

  // 强调色
  root.style.setProperty("--primary", colors.primary);
  root.style.setProperty("--primary-foreground", colors.primaryForeground);
  root.style.setProperty("--secondary", colors.secondary);
  root.style.setProperty("--secondary-foreground", colors.secondaryForeground);
  root.style.setProperty("--muted", colors.muted);
  root.style.setProperty("--muted-foreground", colors.mutedForeground);
  root.style.setProperty("--accent", colors.accent);
  root.style.setProperty("--accent-foreground", colors.accentForeground);

  // 边框与输入
  root.style.setProperty("--border", colors.border);
  root.style.setProperty("--input", colors.input);
  root.style.setProperty("--ring", colors.ring);

  // 侧栏颜色（随主题色变化）
  root.style.setProperty("--sidebar", colors.card);
  root.style.setProperty("--sidebar-foreground", colors.foreground);
  root.style.setProperty("--sidebar-primary", colors.primary);
  root.style.setProperty("--sidebar-primary-foreground", colors.primaryForeground);
  root.style.setProperty("--sidebar-accent", colors.muted);
  root.style.setProperty("--sidebar-accent-foreground", colors.foreground);
  root.style.setProperty("--sidebar-border", colors.border);
  root.style.setProperty("--sidebar-ring", colors.ring);
}

// 应用背景图
function applyBackgroundToDom(bg: BackgroundConfig, effectiveIsDark: boolean) {
  const root = document.documentElement;
  const body = document.body;

  const hasImage = !!bg.imageDataUrl && bg.imageDataUrl.length > 0;

  if (hasImage) {
    root.style.setProperty("--app-bg", `url("${bg.imageDataUrl}")`);
    root.style.setProperty("--app-bg-opacity", bg.opacity.toFixed(2));
    root.style.setProperty("--app-bg-blur", `${Math.round(bg.blur)}px`);
    body.setAttribute("data-app-bg", "active");
  } else {
    root.style.setProperty("--app-bg", "none");
    body.removeAttribute("data-app-bg");
  }
  void effectiveIsDark;
}

// 应用字体大小 —— 核心是让 body 的 font-size 动态变化，
// 同时设置 --app-font-scale 让 text-* / h1-h4 也同步缩放
function applyFontSizeToDom(fontSize: number) {
  const root = document.documentElement;
  const clamped = Math.max(FONT_SIZE_MIN, Math.min(FONT_SIZE_MAX, Math.round(fontSize)));

  // 设置 body 的默认字号（像素）
  document.body.style.fontSize = `${clamped}px`;

  // 设置 CSS 变量供其他地方使用
  root.style.setProperty("--app-font-size", `${clamped}px`);
  root.style.setProperty("--app-font-scale", (clamped / 14).toFixed(3));
}

// 应用主题模式：给 html 加/去 .dark class
function applyThemeModeToDom(mode: ThemeMode) {
  const root = document.documentElement;
  root.classList.toggle("dark", mode === "dark");
}

// ============================================================
// Provider 组件
// ============================================================
export function SettingsProvider({ children }: { children: React.ReactNode }) {
  const [settings, setSettings] = React.useState<LauncherSettings>(DEFAULT_SETTINGS);
  const [ready, setReady] = React.useState(false);

  // 初始化：读取本地存储
  React.useEffect(() => {
    let cancelled = false;
    let merged: LauncherSettings = {
      ...DEFAULT_SETTINGS,
      general: {
        ...DEFAULT_SETTINGS.general,
        language: detectSystemLanguage(),
      },
      appearance: {
        ...DEFAULT_SETTINGS.appearance,
        background: { ...DEFAULT_SETTINGS.appearance.background },
      },
    };

    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<LauncherSettings>;
        merged = {
          general: {
            ...DEFAULT_SETTINGS.general,
            ...(parsed.general ?? {}),
            language: isAppLanguage(parsed.general?.language)
              ? parsed.general.language
              : detectSystemLanguage(),
          },
          appearance: {
            ...DEFAULT_SETTINGS.appearance,
            ...parsed.appearance,
            background: { ...DEFAULT_SETTINGS.appearance.background, ...(parsed.appearance?.background ?? {}) },
          },
        };
      }
    } catch (e) {
      console.warn("[settings] 读取失败，使用默认值", e);
    }

    queueMicrotask(() => {
      if (cancelled) return;
      setSettings(merged);
      setReady(true);
    });

    return () => {
      cancelled = true;
    };
  }, []);

  // 每次设置变化都应用到 DOM 并持久化
  React.useEffect(() => {
    if (!ready) return;

    const isDark = settings.appearance.themeMode === "dark";
    document.documentElement.lang = settings.general.language;
    applyThemeModeToDom(settings.appearance.themeMode);
    applyThemeColorToDom(settings.appearance.themeColor, isDark);
    applyFontSizeToDom(settings.appearance.fontSize);
    applyBackgroundToDom(settings.appearance.background, isDark);

    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch (e) {
      console.warn("[settings] 写入失败", e);
    }
  }, [settings, ready]);

  // 监听 html 上的 .dark class 变化（与 next-themes / mode-toggle 协作）
  React.useEffect(() => {
    if (!ready) return;

    const root = document.documentElement;
    const observer = new MutationObserver(() => {
      const externalIsDark = root.classList.contains("dark");
      applyThemeColorToDom(settings.appearance.themeColor, externalIsDark);
      applyBackgroundToDom(settings.appearance.background, externalIsDark);
    });

    observer.observe(root, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, [settings, ready]);

  const update = React.useCallback(<K extends keyof LauncherSettings>(
    section: K,
    patch: Partial<LauncherSettings[K]>
  ) => {
    setSettings((prev) => ({
      ...prev,
      [section]: { ...prev[section], ...patch } as LauncherSettings[K],
    }));
  }, []);

  const reset = React.useCallback(() => setSettings({
    ...DEFAULT_SETTINGS,
    general: {
      ...DEFAULT_SETTINGS.general,
      language: detectSystemLanguage(),
    },
  }), []);

  const value = React.useMemo<SettingsContextValue>(
    () => ({ settings, update, reset }),
    [settings, update, reset]
  );

  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

export function useSettings() {
  const ctx = React.useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings 必须在 SettingsProvider 内部使用");
  return ctx;
}
