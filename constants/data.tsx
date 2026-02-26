import type { Announcement, InstanceCard } from "@/types";

/**
 * 公告数据
 */
export const ANNOUNCEMENTS: Announcement[] = [
  {
    id: "announcement-1",
    title: "欢迎使用 RTLauncher",
    content: "全新的 Minecraft 启动器，提供现代化的设计和流畅的体验。",
  },
  {
    id: "announcement-2",
    title: "系统更新",
    content: "我们最近发布了新功能，改善了用户体验。",
  },
  {
    id: "announcement-3",
    title: "使用提示",
    content: "查看我们的文档了解如何更好地使用本系统。",
  },
];

/**
 * 实例卡片配置数据
 */
export const INSTANCE_CARDS: InstanceCard[] = [
  {
    id: "mods",
    title: "Mods",
    description: "模组管理中心",
    href: "/instance-settings/mods",
    stats: ["• 已安装：72个模组", "• 更新可用：3个", "• 配置文件编辑"],
    colorFrom: "rgba(16, 185, 129, 0.1)",
    colorTo: "rgba(34, 197, 94, 0.1)",
    iconBgColor: "bg-emerald-50 dark:bg-emerald-900/20",
    iconColor: "text-emerald-600 dark:text-emerald-400",
    icon: (
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"
      />
    ),
  },
  {
    id: "worlds",
    title: "世界",
    description: "存档管理",
    href: "/instance-settings/worlds",
    stats: ["• 游戏存档：6个", "• 最近游戏：RTL World", "• 自动备份"],
    colorFrom: "rgba(251, 146, 60, 0.1)",
    colorTo: "rgba(251, 191, 36, 0.1)",
    iconBgColor: "bg-amber-50 dark:bg-amber-900/20",
    iconColor: "text-amber-600 dark:text-amber-400",
    icon: (
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 104 0 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
      />
    ),
  },
  {
    id: "resources",
    title: "资源包",
    description: "游戏材质管理",
    href: "/instance-settings/resources",
    stats: ["• 当前使用：默认高清", "• 已安装：4个包", "• 资源包排序"],
    colorFrom: "rgba(59, 130, 246, 0.1)",
    colorTo: "rgba(99, 102, 241, 0.1)",
    iconBgColor: "bg-blue-50 dark:bg-blue-900/20",
    iconColor: "text-blue-600 dark:text-blue-400",
    icon: (
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"
      />
    ),
  },
  {
    id: "shaders",
    title: "光影包",
    description: "视觉效果增强",
    href: "/instance-settings/shaders",
    stats: ["• 当前光影：BSL", "• 已安装：3个", "• 性能配置"],
    colorFrom: "rgba(167, 139, 250, 0.1)",
    colorTo: "rgba(168, 85, 247, 0.1)",
    iconBgColor: "bg-violet-50 dark:bg-violet-900/20",
    iconColor: "text-violet-600 dark:text-violet-400",
    icon: (
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
      />
    ),
  },
  {
    id: "screenshots",
    title: "截图",
    description: "游戏截图管理",
    href: "/instance-settings/screenshots",
    stats: ["• 总数：126张", "• 最近截图：今天", "• 快速分享"],
    colorFrom: "rgba(34, 211, 238, 0.1)",
    colorTo: "rgba(20, 184, 166, 0.1)",
    iconBgColor: "bg-cyan-50 dark:bg-cyan-900/20",
    iconColor: "text-cyan-600 dark:text-cyan-400",
    icon: (
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z"
      />
    ),
  },
  {
    id: "schematics",
    title: "投影原理图",
    description: "结构设计管理",
    href: "/instance-settings/schematics",
    stats: ["• 原理图：12个", "• 最近使用：Redstone Castle", "• 快速部署"],
    colorFrom: "rgba(20, 184, 166, 0.1)",
    colorTo: "rgba(34, 211, 238, 0.1)",
    iconBgColor: "bg-teal-50 dark:bg-teal-900/20",
    iconColor: "text-teal-600 dark:text-teal-400",
    icon: (
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
      />
    ),
  },
];
