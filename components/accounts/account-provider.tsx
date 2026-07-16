"use client";

import React, { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { Account, AuthType } from "@/types";
import {
  loginLittleSkin,
  loginLittleSkinWithCredentials,
  loginThirdParty,
  msRequestDeviceCode,
  msPollAndLogin,
  msCancelLogin,
  getSkinBase64,
  type LittleSkinAccount,
  type ThirdPartyAccountList,
  type DeviceCodeInfo,
} from "@/lib/auth";

type LoginState = "idle" | "loading" | "error";

type AccountContextType = {
  profiles: Account[];
  selectedProfile: Account | null;
  selectProfile: (acc: Account) => void;
  removeProfile: (id: string) => void;
  /** LittleSkin OAuth 登录 */
  loginWithLittleSkin: () => Promise<void>;
  /** LittleSkin 账号密码登录（PCL2 风格，无需浏览器），返回玩家列表 */
  loginWithLittleSkinCredentials: (
    username: string,
    password: string
  ) => Promise<LittleSkinAccount[]>;
  /** 选择 LittleSkin 角色后添加账户 */
  addLittleSkinAccount: (account: LittleSkinAccount) => void;
  /** 第三方 Yggdrasil 登录 */
  loginWithThirdParty: (
    url: string,
    user: string,
    pwd: string
  ) => Promise<ThirdPartyAccountList>;
  /** 添加离线账户 */
  addOfflineAccount: (name: string) => void;
  /** 选择第三方角色后添加账户 */
  addThirdPartyAccount: (
    profile: { id: string; name: string },
    accessToken: string,
    yggdrasilUrl: string,
  ) => void;
  /** 微软正版登录 —— 返回 DeviceCodeInfo 后由前端展示，后台继续轮询 */
  loginWithMicrosoft: () => Promise<DeviceCodeInfo>;
  /** 取消微软正版登录（用户关闭对话框时调用） */
  cancelMicrosoftLogin: () => void;
  loginState: LoginState;
  loginError: string | null;
};

const AccountContext = createContext<AccountContextType | undefined>(undefined);

const AUTH_TYPE_LABELS: Record<AuthType, string> = {
  littleskin: "LittleSkin 登录",
  third_party: "第三方登录",
  offline: "离线登录",
  microsoft: "正版登录",
};

const STORAGE_KEY_PROFILES = "rtl_accounts";
const STORAGE_KEY_SELECTED = "rtl_selected_account_id";

function loadProfiles(): Account[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_PROFILES);
    if (raw) return JSON.parse(raw) as Account[];
  } catch {}
  return [];
}

function loadSelectedId(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY_SELECTED);
  } catch {}
  return null;
}

export function AccountProvider({ children }: { children: React.ReactNode }) {
  const [profiles, setProfiles] = useState<Account[]>([]);
  const [selectedProfile, setSelectedProfile] = useState<Account | null>(null);
  const [loginState, setLoginState] = useState<LoginState>("idle");
  const [loginError, setLoginError] = useState<string | null>(null);
  // 微软登录的"取消标志"：每次调用 loginWithMicrosoft 时递增，
  // 如果用户在途中调用 cancelMicrosoftLogin，这个计数器会递增，
  // 使得后续的 .then/.catch 回调不更新状态（避免关闭对话框后仍显示"登录中"）
  const msLoginVersionRef = React.useRef<number>(0);

  // 客户端挂载后从 localStorage 恢复数据，避免 SSR hydration 不匹配
  useEffect(() => {
    const all = loadProfiles();
    const id = loadSelectedId();
    const selected = all.find((p) => p.id === id) ?? all[0] ?? null;
    setProfiles(all);
    setSelectedProfile(selected);
  }, []);

  // 持久化
  useEffect(() => {
    if (profiles.length === 0) return;
    try {
      localStorage.setItem(STORAGE_KEY_PROFILES, JSON.stringify(profiles));
    } catch {}
  }, [profiles]);

  useEffect(() => {
    try {
      if (selectedProfile) {
        localStorage.setItem(STORAGE_KEY_SELECTED, selectedProfile.id);
      } else {
        localStorage.removeItem(STORAGE_KEY_SELECTED);
      }
    } catch {}
  }, [selectedProfile]);

  const selectProfile = useCallback((acc: Account) => {
    setSelectedProfile(acc);
  }, []);

  const removeProfile = useCallback(
    (id: string) => {
      setProfiles((prev) => {
        const next = prev.filter((p) => p.id !== id);
        if (selectedProfile?.id === id) {
          setSelectedProfile(next[0] ?? null);
        }
        return next;
      });
    },
    [selectedProfile]
  );

  // ---- LittleSkin 账号密码登录（PCL2 风格，无需浏览器）----
  const loginWithLittleSkinCredentials = useCallback(
    async (username: string, password: string) => {
      setLoginState("loading");
      setLoginError(null);
      try {
        const accounts = await loginLittleSkinWithCredentials(username, password);
        setLoginState("idle");
        return accounts;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setLoginError(msg);
        setLoginState("error");
        throw e;
      }
    },
    []
  );

  const addLittleSkinAccount = useCallback((account: LittleSkinAccount) => {
    const newAccount: Account = {
      id: `ls-${account.uuid}`,
      name: account.name,
      uuid: account.uuid,
      authType: "littleskin",
      status: "LittleSkin 登录",
      accessToken: account.access_token,
      skinUrl: account.skin_url ?? undefined,
    };
    setProfiles((prev) => {
      const filtered = prev.filter((p) => p.uuid !== account.uuid || p.authType !== "littleskin");
      return [...filtered, newAccount];
    });
    setSelectedProfile(newAccount);
    // 异步获取皮肤 base64（皮肤已经下载到本地）
    if (account.skin_url) {
      getSkinBase64(account.skin_url)
        .then((skinSrc) => {
          setProfiles((prev) =>
            prev.map((p) =>
              p.id === `ls-${account.uuid}` ? { ...p, skinUrl: skinSrc } : p
            )
          );
        })
        .catch(() => {
          // 静默失败
        });
    }
  }, []);

  // ---- LittleSkin OAuth 登录 ----
  const loginWithLittleSkin = useCallback(async () => {
    setLoginState("loading");
    setLoginError(null);
    try {
      const info = await loginLittleSkin();
      const newAccount: Account = {
        id: `ls-${info.uuid}`,
        name: info.name,
        uuid: info.uuid,
        authType: "littleskin",
        status: "LittleSkin 登录",
        accessToken: info.access_token,
        skinUrl: info.skin_url ?? undefined,
      };
      setProfiles((prev) => {
        const filtered = prev.filter(
          (p) => !(p.uuid === info.uuid && p.authType === "littleskin")
        );
        return [...filtered, newAccount];
      });
      setSelectedProfile(newAccount);
      // 异步获取皮肤 base64
      if (info.skin_url) {
        getSkinBase64(info.skin_url)
          .then((skinSrc) => {
            setProfiles((prev) =>
              prev.map((p) =>
                p.id === `ls-${info.uuid}` ? { ...p, skinUrl: skinSrc } : p
              )
            );
          })
          .catch(() => {});
      }
      setLoginState("idle");
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setLoginError(msg);
      setLoginState("error");
    }
  }, []);

  // ---- 第三方 Yggdrasil 登录 (先获取角色列表) ----
  const loginWithThirdParty = useCallback(
    async (url: string, user: string, pwd: string) => {
      setLoginState("loading");
      setLoginError(null);
      try {
        const result = await loginThirdParty(url, user, pwd);
        setLoginState("idle");
        return result;
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        setLoginError(msg);
        setLoginState("error");
        throw e;
      }
    },
    []
  );

  // ---- 选择第三方角色后添加账户 ----
  const addThirdPartyAccount = useCallback(
    (
      profile: { id: string; name: string },
      accessToken: string,
      yggdrasilUrl: string,
    ) => {
      const newAccount: Account = {
        id: `tp-${profile.id}`,
        name: profile.name,
        uuid: profile.id,
        authType: "third_party",
        status: AUTH_TYPE_LABELS.third_party,
        accessToken,
        yggdrasilUrl,
      };
      setProfiles((prev) => {
        const filtered = prev.filter(
          (p) => !(p.uuid === profile.id && p.authType === "third_party")
        );
        return [...filtered, newAccount];
      });
      setSelectedProfile(newAccount);
      // 异步下载并获取皮肤 base64
      if (profile.id) {
        getSkinBase64(profile.id)
          .then((skinSrc) => {
            setProfiles((prev) =>
              prev.map((p) =>
                p.id === `tp-${profile.id}` ? { ...p, skinUrl: skinSrc } : p
              )
            );
          })
          .catch(() => {
            // 静默失败
          });
      }
    },
    []
  );

  // ---- 离线登录 ----
  const addOfflineAccount = useCallback((name: string) => {
    const id = `offline-${Date.now()}`;
    // 基于玩家名生成稳定的 UUID v3（离线认证不需要真实 token）
    const hashStr = `OfflinePlayer:${name}`;
    let h1 = 0xdeadbeef ^ hashStr.length;
    let h2 = 0x41c6ce57 ^ hashStr.length;
    for (let i = 0; i < hashStr.length; i++) {
      const ch = hashStr.charCodeAt(i);
      h1 = Math.imul(h1 ^ ch, 2654435761);
      h2 = Math.imul(h2 ^ ch, 1597334677);
    }
    h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
    h1 = Math.imul(h1 ^ (h2 >>> 13), 3266489909);
    h1 = Math.imul(h1 ^ (h1 >>> 16), 2246822507);
    const hex = (n: number, len: number) => (n >>> 0).toString(16).padStart(len, "0");
    const generatedUuid = `${hex(h1, 8)}-${hex(h2 >>> 16, 4)}-${hex((h2 & 0x0fff) | 0x3000, 4)}-${hex((h2 >>> 8) & 0xffff, 4)}-${hex(h1 & 0xffffffff, 8)}${hex(h2, 8)}`.substring(0, 36);
    const newAccount: Account = {
      id,
      name,
      uuid: generatedUuid,
      authType: "offline",
      status: AUTH_TYPE_LABELS.offline,
      accessToken: "0",
    };
    setProfiles((prev) => [...prev, newAccount]);
    setSelectedProfile(newAccount);
  }, []);

  // ---- 微软正版登录 ----
  const loginWithMicrosoft = useCallback(async (): Promise<DeviceCodeInfo> => {
    // 每次重新开始微软登录时递增版本号
    const currentVersion = ++msLoginVersionRef.current;
    setLoginState("loading");
    setLoginError(null);
    try {
      // 第一步：获取设备代码
      const codeInfo = await msRequestDeviceCode();

      // 第二步：后台轮询 (不阻塞 UI, 异步等后端返回)
      msPollAndLogin(codeInfo.device_code, codeInfo.interval)
        .then(async (info) => {
          // 如果在等待期间用户已取消（版本号不同），忽略这次结果
          if (msLoginVersionRef.current !== currentVersion) return;
          // info.skin_url 现在是 uuid（本地皮肤文件标识）
          let avatarSrc: string | null = null;
          if (info.uuid) {
            try {
              avatarSrc = await getSkinBase64(info.uuid);
            } catch {
              // 皮肤获取失败不影响登录
            }
          }
          // 再次检查取消状态（皮肤获取可能需要时间）
          if (msLoginVersionRef.current !== currentVersion) return;
          const newAccount: Account = {
            id: `ms-${info.uuid}`,
            name: info.name,
            uuid: info.uuid,
            authType: "microsoft",
            status: AUTH_TYPE_LABELS.microsoft,
            accessToken: info.access_token,
            skinUrl: avatarSrc ?? undefined,
          };
          setProfiles((prev) => {
            const filtered = prev.filter(
              (p) => !(p.uuid === info.uuid && p.authType === "microsoft")
            );
            return [...filtered, newAccount];
          });
          setSelectedProfile(newAccount);
          setLoginState("idle");
        })
        .catch((e: unknown) => {
          // 用户主动取消时不更新状态（不显示错误）
          if (msLoginVersionRef.current !== currentVersion) return;
          const msg = e instanceof Error ? e.message : String(e);
          // "已取消登录" 是用户主动操作，不设为 error 状态
          if (msg.includes("已取消登录")) {
            setLoginState("idle");
            setLoginError(null);
            return;
          }
          setLoginError(msg);
          setLoginState("error");
        });

      // 立即返回设备代码给 UI 展示
      return codeInfo;
    } catch (e: unknown) {
      // 如果用户在第一步（获取 device_code）时就已取消，不设置 error 状态
      if (msLoginVersionRef.current !== currentVersion) throw e;
      const msg = e instanceof Error ? e.message : String(e);
      setLoginError(msg);
      setLoginState("error");
      throw e;
    }
  }, []);

  // ---- 取消微软正版登录（用户关闭对话框时调用）----
  const cancelMicrosoftLogin = useCallback(() => {
    // 递增版本号：让之前的 then/catch 回调识别到"已被取消"
    msLoginVersionRef.current++;
    // 设置状态恢复为 idle，不再显示"登录中"
    setLoginState("idle");
    setLoginError(null);
    // 通知后端：停止轮询循环
    msCancelLogin().catch(() => {
      // 静默失败
    });
  }, []);

  return (
    <AccountContext.Provider
      value={{
        profiles,
        selectedProfile,
        selectProfile,
        removeProfile,
        loginWithLittleSkin,
        loginWithLittleSkinCredentials,
        addLittleSkinAccount,
        loginWithThirdParty,
        addOfflineAccount,
        addThirdPartyAccount,
        loginWithMicrosoft,
        cancelMicrosoftLogin,
        loginState,
        loginError,
      }}
    >
      {children}
    </AccountContext.Provider>
  );
}

export function useAccountContext() {
  const ctx = useContext(AccountContext);
  if (!ctx)
    throw new Error("useAccountContext must be used within AccountProvider");
  return ctx;
}