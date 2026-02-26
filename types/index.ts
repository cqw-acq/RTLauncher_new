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
