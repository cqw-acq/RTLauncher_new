/**
 * 账户相关类型定义
 */
export type Account = {
  id: number;
  name: string;
  status: string;
};

/**
 * 实例卡片数据类型
 */
export type InstanceCard = {
  id: string;
  title: string;
  description: string;
  icon: React.ReactNode;
  href: string;
  stats: string[];
  colorFrom: string;
  colorTo: string;
  iconBgColor: string;
  iconColor: string;
};

/**
 * 公告数据类型
 */
export type Announcement = {
  id: string;
  title: string;
  content: string;
};

/**
 * 视图类型
 */
export type ViewType = "home" | "instance";

/**
 * 实例信息类型
 */
export type InstanceInfo = {
  name: string;
  id: string;
  minecraftVersion: string;
  loader: string;
  modsCount: number;
  lastPlayed?: string;
  playTime?: string;
};

/**
 * Minecraft 版本类型
 */
export type MinecraftVersionType =
  | "release"
  | "snapshot"
  | "april_fools"
  | "old_version";

/**
 * 加载器类型
 */
export type LoaderType =
  | "vanilla"
  | "forge"
  | "fabric"
  | "quilt"
  | "neoforge"
  | "liteloader"
  | "optifine";

/**
 * 后端返回的版本信息
 */
export type VersionInfo = {
  id: string;
  releaseTime: string;
};

/**
 * 后端返回的分类版本数据
 * [releases, snapshots, april_fools, old_versions]
 */
export type ClassifiedVersions = [
  VersionInfo[],
  VersionInfo[],
  VersionInfo[],
  VersionInfo[],
];

/**
 * 前端展示用的 Minecraft 版本数据
 */
export type MinecraftVersion = {
  id: string;
  type: MinecraftVersionType;
  releaseDate: string;
  isLatest?: boolean;
};

/**
 * 加载器选项
 */
export type LoaderOption = {
  id: LoaderType;
  name: string;
  description: string;
};

/**
 * 加载器版本数据
 */
export type LoaderVersion = {
  id: string;
  version: string;
  releaseDate: string;
  isRecommended?: boolean;
};
