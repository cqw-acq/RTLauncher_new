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
 * 获取玩家头像（基于本地皮肤文件生成）
 * @param uuid 玩家 UUID
 * @returns data URI （data:image/png;base64,...）
 */
export async function getAvatarBase64(uuid: string): Promise<string> {
  return invoke<string>("get_avatar_base64", { uuid });
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
