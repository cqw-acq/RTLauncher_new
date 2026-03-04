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

/**
 * 游戏启动状态
 */
export type LaunchStatus = "idle" | "preparing" | "launching" | "running" | "stopped" | "error";

/**
 * 游戏启动配置
 */
export type LaunchConfig = {
  /** Minecraft 游戏目录 */
  minecraftPath: string;
  /** Java 可执行文件路径 */
  javaPath: string;
  /** Wrapper 路径 */
  wrapperPath: string;
  /** 最大内存 (MB) */
  maxMemory: string;
  /** 游戏版本名 */
  versionName: string;
  /** 加载器类型: "0"=原版, "1"=Forge/Fabric等 */
  loadType: string;
  /** 加载器名称 */
  loadName: string;
  /** 玩家名称（留空则使用账户名） */
  playerName: string;
  /** 认证令牌 (accessToken) */
  authToken: string;
  /** 玩家 UUID */
  uuid: string;
  /** 游戏窗口宽度 */
  windowWidth: string;
  /** 游戏窗口高度 */
  windowHeight: string;
  /** Authlib Injector 路径 (第三方验证) */
  authlibInjectorPath: string;
  /** Yggdrasil API 地址 (第三方验证) */
  yggdrasilApi: string;
  /** 预取数据 */
  prefetchedData: string;
};

/**
 * 启动日志条目
 */
export type LaunchLogEntry = {
  id: number;
  timestamp: string;
  level: "info" | "warn" | "error";
  message: string;
};
