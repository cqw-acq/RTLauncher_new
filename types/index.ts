/**
 * 登录类型
 */
export type AuthType = "littleskin" | "third_party" | "offline" | "microsoft";

/**
 * 账户相关类型定义
 */
export type Account = {
  id: string;
  name: string;
  uuid: string;
  /** 登录类型 */
  authType: AuthType;
  /** 显示用的状态文本 */
  status: string;
  /** access_token */
  accessToken?: string;
  /** 第三方认证服务器地址 */
  yggdrasilUrl?: string;
  /** 皮肤 URL */
  skinUrl?: string | null;
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
  /** 完整的文件名（包括.jar后缀），用于OptiFine等需要完整文件名的加载器 */
  filename?: string;
  /** 官方下载链接（用于OptiFine双源回退） */
  official_url?: string;
};

/**
 * 游戏启动状态
 */
export type LaunchStatus = "idle" | "preparing" | "launching" | "running" | "stopped" | "error";

/**
 * 存储在 config/launcher.json 中的路径配置
 */
export type LauncherPathsConfig = {
  /** Java 可执行文件路径列表 */
  java_paths: string[];
  /** 当前选中的 Java 路径 */
  selected_java_path: string;
  /** Java安装信息映射 */
  java_installations?: Record<string, {
    path: string;
    version: string;
    major_version: number;
    vendor: string;
    architecture: string;
    java_type: string;
  }>;
  /** 游戏目录路径列表 */
  minecraft_paths: string[];
  /** 当前选中的游戏目录 */
  selected_minecraft_path: string;
  /** 平台默认 .minecraft 路径（只读展示） */
  default_minecraft_path: string;
};

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
  /** 用户自定义 JVM 参数：空格/换行分隔，支持引号包裹；将被插入到默认 JVM 参数之后、主类之前 */
  customJvmArgs: string;
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

/**
 * 启动进度信息
 */
export type LaunchProgress = {
  /** 当前阶段 (0-100) */
  currentStep: number;
  /** 总步骤数 */
  totalSteps: number;
  /** 当前阶段描述 */
  currentStage: string;
  /** 进度百分比 (0-100) */
  percentage: number;
};

export interface LaunchStageTiming {
  /** 阶段 ID */
  id: string;
  /** 阶段名（用户友好） */
  name: string;
  /** 是否完成 */
  completed: boolean;
  /** 进入阶段的时间戳（Date.now ms） */
  enteredAt: number | null;
  /** 完成阶段的时间戳（Date.now ms） */
  completedAt: number | null;
  /** 该阶段的耗时毫秒数 */
  durationMs: number | null;
  /** 该阶段收到的日志数量 */
  logCount: number;
}

export interface Log4jLogEntry {
  timestamp: string;
  level: string;
  message: string;
  logger?: string;
  relatedProblem?: string;
}

export interface LaunchAnalysisReport {
  startedAt: number | null;
  endedAt: number | null;
  totalDurationMs: number | null;
  finalStatus: "idle" | "running" | "stopped" | "error" | "timeout" | "in_progress";
  exitCode: number | null;
  stages: LaunchStageTiming[];
  detectedModCount: number | null;
  detectedLoader: string | null;
  detectedMcVersion: string | null;
  warnCount: number;
  errorCount: number;
  errorSamples: string[];
  failureHints: string[];
  totalLogLines: number;
  log4jLogs: Log4jLogEntry[];
  launchParameters?: string;
}

/**
 * 侧边栏标签页配置
 */
export type SidebarTabConfig = {
  /** 标签页ID */
  id: string;
  /** 标签页名称 */
  name: string;
  /** 是否可见 */
  visible: boolean;
  /** 是否可以隐藏（某些核心功能可能不允许隐藏） */
  canHide: boolean;
  /** 显示顺序 */
  order: number;
};

/**
 * 用户界面配置
 */
export type UIConfig = {
  /** 侧边栏标签页配置 */
  sidebarTabs: SidebarTabConfig[];
};

/**
 * 多人联机模式
 */
export type MultiplayerMode = "host" | "join";

/**
 * vm_list_dir 返回的条目信息
 */
export type DirEntry = {
  /** 文件或目录名（不含路径） */
  name: string;
  /** 是否为目录 */
  is_dir: boolean;
  /** 文件扩展名（小写，不含点；目录为空字符串） */
  extension: string;
  /** 文件大小（字节），目录为 0 */
  size: number;
};

// ── version_management 后端返回类型 ──────────────────────────────

/**
 * vm_scan_instances 返回的单个实例数据
 */
export type InstanceData = {
  /** 实例目录名（同时作为实例关键标识符） */
  name: string;
  /** Minecraft 版本号（从版本 JSON 的 inheritsFrom 字段推断） */
  minecraft_version: string;
  /** 加载器类型（Vanilla / Fabric / Forge / Quilt / NeoForge / LiteLoader） */
  loader: string;
  /** mods/ 目录中的 mod 文件数量 */
  mods_count: number;
};

/**
 * vm_find_resource_packs 返回的单个材质包信息
 */
export type ResourcePackInfo = {
  /** 材质包目录名 */
  name: string;
  /** pack.png 的绝对路径（不存在时为空字符串） */
  icon_path: string;
  /** 基于 pack_format 推断的 MC 版本范围描述 */
  mc_version_range: string;
};

/**
 * vm_parse_level_dat 返回的 level.dat 解析结果
 */
export type LevelDatInfo = {
  /** 世界种子（字符串格式的整数） */
  seed: string;
  keep_inventory: boolean;
  mob_griefing: boolean;
  do_fire_tick: boolean;
  allow_commands: boolean;
};