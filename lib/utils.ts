import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"
import type { LoaderType } from "@/types"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * 玩家头像纯色背景调色板
 * 用于根据玩家名生成稳定的纯色背景（不再使用整张皮肤文件）
 */
const AVATAR_COLORS = [
  "bg-red-500",
  "bg-orange-500",
  "bg-amber-500",
  "bg-lime-600",
  "bg-green-500",
  "bg-emerald-500",
  "bg-teal-500",
  "bg-cyan-500",
  "bg-sky-500",
  "bg-blue-500",
  "bg-indigo-500",
  "bg-violet-500",
  "bg-purple-500",
  "bg-fuchsia-500",
  "bg-pink-500",
  "bg-rose-500",
];

/**
 * 根据玩家名生成稳定的纯色背景类名
 */
export function getAvatarColor(name: string): string {
  if (!name) return AVATAR_COLORS[9];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return AVATAR_COLORS[Math.abs(hash) % AVATAR_COLORS.length];
}

/**
 * 从玩家名提取大写首字母作为头像文字
 */
export function getAvatarInitials(name: string): string {
  if (!name) return "?";
  return name.charAt(0).toUpperCase();
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

/** 模组加载器互斥组：同一时刻最多选择一个 */
export const MOD_LOADER_GROUP: LoaderType[] = [
  "forge",
  "neoforge",
  "fabric",
  "quilt",
  "liteloader",
];

/** 与 OptiFine 兼容的加载器（参考 PCL/HMCL：可叠加 Forge/NeoForge，不能与 Fabric/Quilt/LiteLoader 组合） */
const OPTIFINE_COMPATIBLE_LOADERS: LoaderType[] = ["forge", "neoforge"];

/**
 * 判断两个加载器是否可以同时选择。
 * - 模组加载器（Forge/NeoForge/Fabric/Quilt/LiteLoader）互斥
 * - OptiFine 可与 Forge/NeoForge 叠加，不能与 Fabric/Quilt/LiteLoader 组合
 */
export function isLoaderPairCompatible(a: LoaderType, b: LoaderType): boolean {
  if (a === b) return true;
  if (a === "vanilla" || b === "vanilla") return true;
  if (MOD_LOADER_GROUP.includes(a) && MOD_LOADER_GROUP.includes(b)) return false;
  if (a === "optifine") return OPTIFINE_COMPATIBLE_LOADERS.includes(b);
  if (b === "optifine") return OPTIFINE_COMPATIBLE_LOADERS.includes(a);
  return true;
}

/**
 * 判断加载器在当前已选集合下是否可勾选。
 * 返回 { allowed, reason }，reason 用于禁用时提示不兼容原因。
 */
export function canSelectLoader(
  mcVersion: string,
  loader: LoaderType,
  selected: LoaderType[]
): { allowed: boolean; reason?: string } {
  if (loader !== "vanilla" && !isLoaderCompatible(mcVersion, loader)) {
    return { allowed: false, reason: "当前 Minecraft 版本不支持该加载器" };
  }
  for (const other of selected) {
    if (other === loader) continue;
    if (!isLoaderPairCompatible(other, loader)) {
      return { allowed: false, reason: `与「${other}」不兼容` };
    }
  }
  return { allowed: true };
}
