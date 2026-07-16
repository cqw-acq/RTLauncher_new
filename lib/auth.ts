/**
 * 前端 Auth API 层 —— 通过 Tauri invoke 调用后端 auth 命令
 */
import { invoke } from "@tauri-apps/api/core";

// ======================== 类型定义 ========================

/** 后端返回的账户信息 (LittleSkin) */
export type AccountInfo = {
  name: string;
  uuid: string;
  auth_type: string;
  access_token: string;
  skin_url: string | null;
};

/** LittleSkin 账号密码登录结果（一个账号可能多个玩家） */
export type LittleSkinAccount = {
  name: string;
  uuid: string;
  access_token: string;
  skin_url: string | null;
};

/** 第三方认证服务器的角色 */
export type ThirdPartyProfile = {
  id: string;
  name: string;
};

/** 第三方登录后获取到的角色列表 */
export type ThirdPartyAccountList = {
  access_token: string;
  profiles: ThirdPartyProfile[];
};

/** 微软设备代码信息 */
export type DeviceCodeInfo = {
  user_code: string;
  verification_uri: string;
  device_code: string;
  interval: number;
  expires_in: number;
};

// ======================== API 调用 ========================

/**
 * LittleSkin OAuth 登录
 * 会自动打开浏览器进行授权，登录成功后返回账户信息
 */
export async function loginLittleSkin(): Promise<AccountInfo> {
  return invoke<AccountInfo>("useMethod");
}

/**
 * LittleSkin 账号密码登录（PCL2 风格，无需浏览器）
 * 返回玩家角色列表（一个 LittleSkin 账号可能关联多个玩家）
 * @param username LittleSkin 账号（邮箱或用户名）
 * @param password LittleSkin 密码
 */
export async function loginLittleSkinWithCredentials(
  username: string,
  password: string
): Promise<LittleSkinAccount[]> {
  return invoke<LittleSkinAccount[]>("use_method_with_credentials", {
    username,
    password,
  });
}

/**
 * 第三方 Yggdrasil 服务器验证
 * 传入认证服务器 URL，返回 base64 编码的服务器信息（用于 authlib-injector）
 * @param url 认证服务器的 API 根地址
 */
export async function verifyThirdPartyServer(url: string): Promise<string> {
  return invoke<string>("thirdPartyLogin", { url });
}

/**
 * 第三方 Yggdrasil 服务器登录
 * 传入认证服务器 URL、用户名、密码，返回 accessToken 和可用角色列表
 */
export async function loginThirdParty(
  url: string,
  user: string,
  pwd: string
): Promise<ThirdPartyAccountList> {
  return invoke<ThirdPartyAccountList>("getAccountList", { url, user, pwd });
}

/**
 * 获取玩家皮肤（第三方 Yggdrasil）
 * @param url 认证服务器 API 根地址
 * @param uuid 玩家 UUID
 * @returns 皮肤本地路径
 */
export async function getPlayerSkin(url: string, uuid: string): Promise<string> {
  return invoke<string>("getPlayerSkin", { url, uuid });
}

/**
 * 获取玩家皮肤 base64（本地存储在 RTL/config/skins/{uuid}.png）
 * 用于前端 3D 皮肤展示
 * @param uuid 玩家 UUID
 * @returns data URI (data:image/png;base64,...)
 */
export async function getSkinBase64(uuid: string): Promise<string> {
  return invoke<string>("get_skin_base64", { uuid });
}

// ======================== 微软正版登录 ========================

/**
 * 微软登录第一步：请求设备代码
 * 返回 user_code（展示给用户）和 verification_uri（让用户打开的网址）
 */
export async function msRequestDeviceCode(): Promise<DeviceCodeInfo> {
  return invoke<DeviceCodeInfo>("ms_request_device_code");
}

/**
 * 微软登录第二步：轮询等待用户完成授权
 * 完成后自动执行 Xbox / XSTS / Minecraft 认证链，返回账户信息
 * @param deviceCode 设备代码
 * @param interval 轮询间隔（秒）
 */
export async function msPollAndLogin(
  deviceCode: string,
  interval: number
): Promise<AccountInfo> {
  return invoke<AccountInfo>("ms_poll_and_login", {
    deviceCode,
    interval,
  });
}

/**
 * 用户关闭登录对话框时调用：让后台的轮询循环中止
 */
export async function msCancelLogin(): Promise<void> {
  return invoke("ms_cancel_login");
}

// ======================== 微软正版：皮肤/披风管理 ========================

/** 单个皮肤信息 */
export type MCSkinInfo = {
  id: string;
  state: string; // ACTIVE / INACTIVE
  url: string;
  variant: string; // classic / slim
  alias: string | null;
};

/** 单个披风信息 */
export type MCCapeInfo = {
  id: string;
  state: string; // ACTIVE / INACTIVE
  url: string;
  alias: string | null;
};

/** 完整皮肤/披风列表 */
export type MCSkinCapeProfile = {
  skins: MCSkinInfo[];
  capes: MCCapeInfo[];
};

/**
 * 获取当前微软账号的所有皮肤与披风列表（基于 Minecraft Services API）
 * @param accessToken 微软登录得到的 access_token
 */
export async function msGetSkinsAndCapes(
  accessToken: string
): Promise<MCSkinCapeProfile> {
  return invoke<MCSkinCapeProfile>("ms_get_skins_and_capes", { accessToken });
}

/**
 * 上传新皮肤（PNG base64 编码），上传后自动设为当前皮肤
 * @param accessToken 微软登录得到的 access_token
 * @param pngBase64 PNG 图像的 base64 编码（不带 data:image/png;base64, 前缀）
 * @param variant "classic"（默认）或 "slim"（细胳膊）
 */
export async function msUploadSkin(
  accessToken: string,
  pngBase64: string,
  variant: "classic" | "slim"
): Promise<string> {
  return invoke<string>("ms_upload_skin", {
    accessToken,
    pngBase64,
    variant,
  });
}

/**
 * 激活/切换到指定皮肤（从已有皮肤列表中选择）
 * @param accessToken 微软登录得到的 access_token
 * @param skinId 皮肤 ID
 * @param variant "classic" 或 "slim"
 */
export async function msActivateSkin(
  accessToken: string,
  skinId: string,
  variant: "classic" | "slim"
): Promise<void> {
  return invoke<void>("ms_activate_skin", {
    accessToken,
    skinId,
    variant,
  });
}

/**
 * 删除指定皮肤
 * @param accessToken 微软登录得到的 access_token
 * @param skinId 皮肤 ID
 */
export async function msDeleteSkin(
  accessToken: string,
  skinId: string
): Promise<void> {
  return invoke<void>("ms_delete_skin", { accessToken, skinId });
}

/**
 * 设置激活披风（capeId 为空字符串时取消激活当前披风）
 * @param accessToken 微软登录得到的 access_token
 * @param capeId 披风 ID（空字符串 = 取消激活）
 */
export async function msSetActiveCape(
  accessToken: string,
  capeId: string
): Promise<void> {
  return invoke<void>("ms_set_active_cape", { accessToken, capeId });
}