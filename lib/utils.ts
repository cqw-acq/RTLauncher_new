import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"
import type { LoaderType } from "@/types"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * 解析 Minecraft 版本号为数字数组
 * 例如 "1.20.2" → [1, 20, 2]，"1.14" → [1, 14]
 * 非标准版本号（快照、远古版等）返回 null
 */
function parseMcVersion(version: string): number[] | null {
  // 只匹配 X.Y 或 X.Y.Z 格式的正式版本号
  const match = version.match(/^(\d+)\.(\d+)(?:\.(\d+))?$/);
  if (!match) return null;
  const parts = [Number(match[1]), Number(match[2])];
  if (match[3] !== undefined) parts.push(Number(match[3]));
  return parts;
}

/**
 * 比较两个 Minecraft 版本号
 * 返回: 负数 = a < b, 0 = 相等, 正数 = a > b
 */
export function compareMcVersions(a: string, b: string): number {
  const pa = parseMcVersion(a);
  const pb = parseMcVersion(b);
  if (!pa || !pb) return 0;
  const len = Math.max(pa.length, pb.length);
  for (let i = 0; i < len; i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * 各加载器支持的 Minecraft 版本范围
 * min: 最低支持版本（含），max: 最高支持版本（含，不填表示无上限）
 */
const LOADER_VERSION_RANGES: Record<LoaderType, { min?: string; max?: string }> = {
  vanilla:    {},                                  // 所有版本
  forge:      { min: "1.5.2" },                    // 1.5.2+
  liteloader: { min: "1.5.2", max: "1.12.2" },    // 1.5.2 ~ 1.12.2
  neoforge:   { min: "1.20.2" },                   // 1.20.2+
  fabric:     { min: "1.14" },                     // 1.14+
  quilt:      { min: "1.14.4" },                   // 1.14.4+
  optifine:   { min: "1.0" },                      // 几乎所有正式版
};

/**
 * 判断指定加载器是否兼容某个 MC 版本
 * 非标准版本号（快照、远古版等）仅显示 Vanilla
 */
export function isLoaderCompatible(mcVersion: string, loader: LoaderType): boolean {
  if (loader === "vanilla") return true;

  const parsed = parseMcVersion(mcVersion);
  if (!parsed) return false; // 非标准版本号，只允许 vanilla

  const range = LOADER_VERSION_RANGES[loader];
  if (range.min && compareMcVersions(mcVersion, range.min) < 0) return false;
  if (range.max && compareMcVersions(mcVersion, range.max) > 0) return false;
  return true;
}
