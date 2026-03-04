"use client";

import React, { createContext, useCallback, useContext, useEffect, useState } from "react";
import type { Account, AuthType } from "@/types";
import {
  loginLittleSkin,
  loginThirdParty,
  msRequestDeviceCode,
  msPollAndLogin,
  getAvatarBase64,
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

  // ---- LittleSkin 登录 ----
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
        status: AUTH_TYPE_LABELS.littleskin,
        accessToken: info.access_token,
        skinUrl: info.skin_url,
      };
      setProfiles((prev) => {
        // 去重：同 uuid 替换
        const filtered = prev.filter((p) => p.uuid !== info.uuid || p.authType !== "littleskin");
        return [...filtered, newAccount];
      });
      setSelectedProfile(newAccount);
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
      // 异步生成头像
      if (profile.id) {
        getAvatarBase64(profile.id)
          .then((avatarSrc) => {
            setProfiles((prev) =>
              prev.map((p) =>
                p.id === `tp-${profile.id}` ? { ...p, skinUrl: avatarSrc } : p
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
    const newAccount: Account = {
      id,
      name,
      uuid: "",
      authType: "offline",
      status: AUTH_TYPE_LABELS.offline,
    };
    setProfiles((prev) => [...prev, newAccount]);
    setSelectedProfile(newAccount);
  }, []);

  // ---- 微软正版登录 ----
  const loginWithMicrosoft = useCallback(async (): Promise<DeviceCodeInfo> => {
    setLoginState("loading");
    setLoginError(null);
    try {
      // 第一步：获取设备代码
      const codeInfo = await msRequestDeviceCode();

      // 第二步：后台轮询 (不阻塞 UI, 异步等后端返回)
      msPollAndLogin(codeInfo.device_code, codeInfo.interval)
        .then(async (info) => {
          let avatarSrc: string | null = info.skin_url ?? null;
          if (!avatarSrc && info.uuid) {
            try {
              avatarSrc = await getAvatarBase64(info.uuid);
            } catch {
              // 头像生成失败不影响登录
            }
          }
          const newAccount: Account = {
            id: `ms-${info.uuid}`,
            name: info.name,
            uuid: info.uuid,
            authType: "microsoft",
            status: AUTH_TYPE_LABELS.microsoft,
            accessToken: info.access_token,
            skinUrl: avatarSrc,
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
          const msg = e instanceof Error ? e.message : String(e);
          setLoginError(msg);
          setLoginState("error");
        });

      // 立即返回设备代码给 UI 展示
      return codeInfo;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setLoginError(msg);
      setLoginState("error");
      throw e;
    }
  }, []);

  return (
    <AccountContext.Provider
      value={{
        profiles,
        selectedProfile,
        selectProfile,
        removeProfile,
        loginWithLittleSkin,
        loginWithThirdParty,
        addOfflineAccount,
        addThirdPartyAccount,
        loginWithMicrosoft,
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
