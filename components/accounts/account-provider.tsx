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
  redownloadLittleSkinSkin,
  msSilentRefreshAccount,
  msHasAccountInDb,
  deleteCachedSkin,
  microsoftProbeAccountLogin,
  type LittleSkinAccount,
  type ThirdPartyAccountList,
  type DeviceCodeInfo,
} from "@/lib/auth";
import { LoginDialog } from "@/components/accounts/login-dialog";

type LoginState = "idle" | "loading" | "error";

type AccountContextType = {
  profiles: Account[];
  selectedProfile: Account | null;
  selectProfile: (acc: Account) => void;
  removeProfile: (id: string) => void;
  /** 更新单个账户信息（例如刷新皮肤URL） */
  updateProfile: (id: string, patch: Partial<Account>) => void;
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
  /**
   * 自动弹出的强制微软登录 DeviceCode。
   * 启动器打开时，如果数据库有正版账号且 access_token 失效、refresh_token 也无法刷新，
   * 会自动发起一次微软正版登录流程，并将 DeviceCode 信息存于此状态，UI 层应自动弹出登录对话框展示。
   * UI 使用后应调用 clearForcedMicrosoftDeviceCode() 清理。
   */
  forcedMicrosoftDeviceCode: DeviceCodeInfo | null;
  /** 是否进入"强制登录"模式：弹窗不可关闭、不可切换 tab，必须完成微软登录 */
  forcedMicrosoftLoginMode: boolean;
  /** 强制登录的原因说明（展示给玩家） */
  forcedMicrosoftLoginMessage: string | null;
  /** 清理强制微软登录的 DeviceCode 状态（登录对话框关闭时调用）*/
  clearForcedMicrosoftDeviceCode: () => void;
  /** 手动打开登录对话框（AccountSwitcher 里"添加账户"按钮点下时设为 true；关闭时设为 false） */
  manualLoginOpen: boolean;
  setManualLoginOpen: (open: boolean) => void;
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
  const [forcedMicrosoftDeviceCode, setForcedMicrosoftDeviceCode] = useState<DeviceCodeInfo | null>(null);
  const [forcedMicrosoftLoginMode, setForcedMicrosoftLoginMode] = useState<boolean>(false);
  const [forcedMicrosoftLoginMessage, setForcedMicrosoftLoginMessage] = useState<string | null>(null);
  // 手动打开的登录对话框（点"添加账户"按钮时）—— 统一由 Provider 控制对话框是否显示
  const [manualLoginOpen, setManualLoginOpen] = useState<boolean>(false);
  // 微软登录的"取消标志"：每次调用 loginWithMicrosoft 时递增，
  // 如果用户在途中调用 cancelMicrosoftLogin，这个计数器会递增，
  // 使得后续的 .then/.catch 回调不更新状态（避免关闭对话框后仍显示"登录中"）
  const msLoginVersionRef = React.useRef<number>(0);

  // 客户端挂载后从 localStorage 恢复数据，避免 SSR hydration 不匹配
  // 同时刷新 LittleSkin 账户的皮肤，确保显示最新的皮肤
  // 对于微软正版账户：利用"皮肤与披风"模块试探 access_token 是否仍然有效，
  // 如果失效则先用 official.rs 的 refresh_token 静默刷新，再失败就强制重新登录
  useEffect(() => {
    const all = loadProfiles();
    const id = loadSelectedId();
    const selected = all.find((p) => p.id === id) ?? all[0] ?? null;
    setProfiles(all);
    setSelectedProfile(selected);

    // 挂载后异步刷新所有 LittleSkin 账户的皮肤
    // 确保用户在 LittleSkin 网站更换皮肤后，重新打开启动器能看到最新皮肤
    all.forEach((profile) => {
      if (profile.authType === "littleskin" && profile.uuid) {
        const pid = profile.id;
        const puuid = profile.uuid;
        (async () => {
          try {
            // 先尝试重新下载皮肤（用户可能在网站上换了皮肤）
            await redownloadLittleSkinSkin(puuid);
            // 下载成功后，读取并更新皮肤显示
            const skinSrc = await getSkinBase64(puuid);
            setProfiles((prev) =>
              prev.map((p) =>
                p.id === pid ? { ...p, skinUrl: skinSrc } : p
              )
            );
          } catch {
            // 刷新失败不影响，保持原来的皮肤（如果有的话）
          }
        })();
      }
    });

    // ── 微软正版账号健康检查（直接使用【皮肤与披风】模块的 msGetSkinsAndCapes 检测） ──
    // 用户要求：不用"自己那一套验证"，每次启动器打开后，有正版账号就调用皮肤与披风那套方法，
    // 看看有没有"无法获取皮肤和披风"的异常情况；有异常就尝试刷新，再不行就强制登录。
    // 所以这里的检测逻辑 = 和皮肤与披风 loadProfile 完全一致地调用 msGetSkinsAndCapes(accessToken)，
    // 只有它抛异常我们才视为"账号有问题"，其他任何情况都视为正常。
    let forcedLoginTriggered = false;

    // 通用：Promise 硬超时包装器（避免网络慢时整段流程卡住）
    const withTimeout = <T,>(p: Promise<T>, ms: number, tag: string): Promise<T> =>
      Promise.race<T>([
        p,
        new Promise<T>((_, rej) => setTimeout(() => rej(new Error(`${tag}_TIMEOUT`)), ms)),
      ]);

    all.forEach((profile) => {
      if (profile.authType !== "microsoft" || !profile.uuid) return;
      const pid = profile.id;
      const puuid = profile.uuid;
      const pName = profile.name;

      (async () => {
        // 1) 先确认 official.rs 数据库是否真的有这个账号（用户要求：只有数据库记载的才做试探）
        console.log(
          `[正版检测-前端] ========== 开始检测微软正版账号：${pName} (UUID=${puuid}) ==========`
        );
        let inDb = false;
        try {
          inDb = await withTimeout(msHasAccountInDb(puuid), 1500, "DB_CHECK");
        } catch (e) {
          console.log(
            `[正版检测-前端] 步骤1：✗ 查询数据库异常:`,
            e instanceof Error ? e.message : String(e)
          );
          return;
        }
        if (!inDb) {
          console.log(
            `[正版检测-前端] 步骤1：✗ official.rs 数据库中未记载该账号（可能已被删除/失效），正在清理残留的皮肤文件和前端账号记录...`
          );
          // 先清理本地磁盘上的皮肤缓存文件
          try {
            const deletedSkinFiles = await deleteCachedSkin(puuid);
            console.log(
              `[正版检测-前端] 步骤1：✓ 清理磁盘皮肤缓存完成，共删除 ${deletedSkinFiles} 个皮肤文件`
            );
          } catch (skinErr) {
            console.log(
              `[正版检测-前端] 步骤1：清理皮肤文件出错（非致命，继续）:`,
              skinErr instanceof Error ? skinErr.message : String(skinErr)
            );
          }
          // 从 profiles 中删除这个残留的微软账号（皮肤信息也是跟着 profile 走的，一并清理）
          setProfiles((prev) => {
            const next = prev.filter((p) => p.id !== pid);
            // 如果删除的是当前选中的账号，把 selectedProfile 切到第一个或 null
            if (selectedProfile?.id === pid) {
              setSelectedProfile(next[0] ?? null);
            }
            if (next.length !== prev.length) {
              console.log(
                `[正版检测-前端] 步骤1：✓ 已从前端移除残留的微软账号：${pName} (${pid})，共 ${prev.length - next.length} 条记录`
              );
            }
            return next;
          });
          return;
        }
        console.log(
          `[正版检测-前端] 步骤1：✓ official.rs 数据库中存在该账号记录`
        );

        // 2) ⚠️  就用"皮肤与披风"那一套方法：直接调 microsoftProbeAccountLogin(accessToken) ⚠️
        //    和用户手动点进"皮肤与披风" → 组件内部 loadProfile → microsoftProbeAccountLogin 完全一样。
        //    任何异常（accessToken 为 null / 返回 access_token 不存在 / HTTP 401 / 网络超时等）
        //    都视为"无法获取皮肤和披风的异常情况" → 需要进一步处理。
        //    注意：microsoftProbeAccountLogin 本身就是 {ok, error} 结构，不会抛出异常。
        console.log(
          `[正版检测-前端] 步骤2：使用【皮肤与披风】模块的 microsoftProbeAccountLogin 试探 access_token...`
        );
        let probeFailed = false;
        let probeErrorMsg = "";
        try {
          // 如果 accessToken 为空，microsoftProbeAccountLogin 的前端 impl 会立刻返回
          // { ok: false, error: "账户 access_token 不存在，请重新登录" }，
          // 和皮肤与披风点击进去看到的完全一致（零网络延迟）。
          const probeResult = await withTimeout(
            microsoftProbeAccountLogin(profile.accessToken),
            3000,
            "SKIN_CAPE_PROBE"
          );
          if (!probeResult.ok) {
            probeFailed = true;
            probeErrorMsg = probeResult.error;
          }
        } catch (e: unknown) {
          // 超时等异常也视为探测失败
          probeFailed = true;
          probeErrorMsg = e instanceof Error ? e.message : String(e);
        }

        // 能正常返回皮肤/披风 → 账号没问题，跳过
        if (!probeFailed) {
          console.log(
            `[正版检测-前端] 步骤2：✓ 皮肤与披风模块试探成功，账号一切正常，无需任何操作`
          );
          return;
        }
        console.log(
          `[正版检测-前端] 步骤2：✗ 皮肤与披风模块试探失败 →`,
          probeErrorMsg
        );

        // 用户明确要求：如果【皮肤与披风】模块返回"账户 access_token 不存在，请重新登录"
        // 就直接调用 official.rs 强制玩家重新登录正版（跳过 refresh_token 静默刷新）
        const isMissingAccessToken =
          probeErrorMsg.includes("账户 access_token 不存在") ||
          probeErrorMsg.includes("请重新登录");

        if (isMissingAccessToken) {
          console.log(
            `[正版检测-前端] 步骤2：⚠ 命中"access_token 不存在"条件 → 根据用户要求，跳过 refresh_token 静默刷新，直接进入强制重新登录流程！`
          );
        }

        // 3) 如果不是"纯缺 token"，就尝试用 official.rs 的 refresh_token 静默刷新一次
        //    （纯缺 token 意味着 refresh_token 可能也没了，没必要等 8 秒）
        let refreshedOk = false;
        if (!isMissingAccessToken) {
          console.log(
            `[正版检测-前端] 步骤3：尝试调用 official.rs msSilentRefreshAccount 静默刷新...`
          );
          try {
            const refreshed = await withTimeout(
              msSilentRefreshAccount(puuid),
              8000,
              "SILENT_REFRESH"
            );
            if (refreshed && refreshed.access_token) {
              let avatarSrc: string | undefined = undefined;
              try {
                avatarSrc = (await getSkinBase64(puuid)) || undefined;
              } catch {}
              setProfiles((prev) =>
                prev.map((p) =>
                  p.id === pid
                    ? {
                        ...p,
                        name: refreshed.name || pName,
                        accessToken: refreshed.access_token,
                        skinUrl: avatarSrc ?? p.skinUrl,
                      }
                    : p
                )
              );
              refreshedOk = true;
              console.log(
                `[正版检测-前端] 步骤3：✓ 静默刷新成功！新 access_token 已应用，玩家名=${refreshed.name || pName}`
              );
            }
          } catch (e) {
            console.log(
              `[正版检测-前端] 步骤3：✗ 静默刷新失败:`,
              e instanceof Error ? e.message : String(e)
            );
            // 静默刷新失败 —— 继续走强制登录
          }
        }

        if (refreshedOk) return;

        // 4) 【皮肤与披风获取异常】 → 强制重新登录
        //    只允许一个账号触发（避免多个坏账号同时弹出多个登录窗）
        if (forcedLoginTriggered) {
          console.log(
            `[正版检测-前端] 步骤4：已有其他账号正在强制登录，跳过当前账号`
          );
          return;
        }
        forcedLoginTriggered = true;
        console.log(
          `[正版检测-前端] 步骤4：⟡ 开始强制执行 official.rs 微软设备码登录流程...`
        );

        try {
          const currentVersion = ++msLoginVersionRef.current;
          setLoginState("loading");
          setLoginError(null);

          // 先开强制登录弹窗，再请求 DeviceCode，避免看起来"卡住"
          setForcedMicrosoftLoginMode(true);
          setForcedMicrosoftLoginMessage(
            `检测到你的微软正版账号「${pName}」无法正常获取皮肤与披风（${probeErrorMsg}）。` +
              `这通常是由于登录凭据失效（长时间未使用或在其他设备/启动器上登录过）导致的。` +
              `必须完成微软正版账号重新登录后才能继续使用启动器。`
          );
          setForcedMicrosoftDeviceCode(null);

          // 后台异步获取 DeviceCode（加 10 秒硬超时）
          let codeInfo: DeviceCodeInfo | null = null;
          try {
            codeInfo = await withTimeout(
              msRequestDeviceCode(),
              10000,
              "DEVICE_CODE_REQUEST"
            );
            console.log(
              `[正版检测-前端] 步骤4：✓ 获取设备授权码成功 user_code=${codeInfo.user_code}`
            );
          } catch (reqErr) {
            const msg =
              reqErr instanceof Error ? reqErr.message : String(reqErr);
            console.log(
              `[正版检测-前端] 步骤4：✗ 获取设备授权码失败:`,
              msg
            );
            if (msLoginVersionRef.current === currentVersion) {
              setLoginError(
                `获取验证码失败（${msg.includes("TIMEOUT") ? "网络超时" : msg}），请检查网络连接或稍后重试。`
              );
            }
            return;
          }

          if (msLoginVersionRef.current !== currentVersion) return;
          setForcedMicrosoftDeviceCode(codeInfo);

          // 后台轮询：登录成功后自动更新前端账号信息
          msPollAndLogin(codeInfo.device_code, codeInfo.interval)
            .then(async (info) => {
              if (msLoginVersionRef.current !== currentVersion) return;
              let avatarSrc: string | null = null;
              if (info.uuid) {
                try {
                  avatarSrc = await getSkinBase64(info.uuid);
                } catch {}
              }
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
              // 登录成功：退出"强制登录"模式
              setForcedMicrosoftLoginMode(false);
              setForcedMicrosoftLoginMessage(null);
              console.log(
                `[正版检测-前端] 步骤4：✅ 强制重新登录成功！玩家=${info.name}, UUID=${info.uuid}`
              );
            })
            .catch((e: unknown) => {
              if (msLoginVersionRef.current !== currentVersion) return;
              const msg = e instanceof Error ? e.message : String(e);
              console.log(
                `[正版检测-前端] 步骤4：✗ 强制重新登录失败:`,
                msg
              );
              if (msg.includes("已取消登录")) {
                // 强制登录模式下不允许取消；保留强制模式状态，只把 loading 去掉
                setLoginState("idle");
                setLoginError(null);
                return;
              }
              setLoginError(msg);
              setLoginState("error");
            });
        } catch (e: unknown) {
          const msg = e instanceof Error ? e.message : String(e);
          console.log(`[正版检测-前端] 步骤4：✗ 强制登录异常:`, msg);
          setLoginError(msg);
          setLoginState("error");
        }
      })();
    });
  }, []);

  const clearForcedMicrosoftDeviceCode = useCallback(() => {
    setForcedMicrosoftDeviceCode(null);
    setForcedMicrosoftLoginMode(false);
    setForcedMicrosoftLoginMessage(null);
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
    if (acc.uuid && !acc.skinUrl && (acc.authType === "microsoft" || acc.authType === "littleskin")) {
      const pid = acc.id;
      const puuid = acc.uuid;
      const pAuthType = acc.authType;
      (async () => {
        try {
          if (pAuthType === "littleskin") {
            await redownloadLittleSkinSkin(puuid);
          }
          const skinSrc = await getSkinBase64(puuid);
          setProfiles((prev) =>
            prev.map((p) => (p.id === pid ? { ...p, skinUrl: skinSrc } : p))
          );
          setSelectedProfile((curr) => (curr && curr.id === pid ? { ...curr, skinUrl: skinSrc } : curr));
        } catch {
          // 皮肤获取失败，静默忽略
        }
      })();
    }
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

  const updateProfile = useCallback(
    (id: string, patch: Partial<Account>) => {
      setProfiles((prev) => {
        const updated = prev.map((p) => (p.id === id ? { ...p, ...patch } : p));
        // 如果更新的是当前选中的账户，也同步更新 selectedProfile
        if (selectedProfile?.id === id) {
          setSelectedProfile((curr) => (curr ? { ...curr, ...patch } : curr));
        }
        return updated;
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
      yggdrasilUrl: "https://littleskin.cn/api/yggdrasil",
      skinUrl: account.skin_url ?? undefined,
    };
    setProfiles((prev) => {
      const filtered = prev.filter((p) => p.uuid !== account.uuid || p.authType !== "littleskin");
      return [...filtered, newAccount];
    });
    setSelectedProfile(newAccount);
    // 异步获取皮肤 base64（皮肤已经下载到本地）
    if (account.skin_url) {
      const accountId = `ls-${account.uuid}`;
      const skinUrlKey = account.skin_url;
      const accountUuid = account.uuid;
      // 使用 IIFE 包裹，避免 .catch(async fn) 反模式
      (async () => {
        try {
          const skinSrc = await getSkinBase64(skinUrlKey);
          setProfiles((prev) =>
            prev.map((p) => (p.id === accountId ? { ...p, skinUrl: skinSrc } : p))
          );
        } catch {
          // 第一次失败：尝试重新下载皮肤
          try {
            await redownloadLittleSkinSkin(accountUuid);
            const skinSrc = await getSkinBase64(skinUrlKey);
            setProfiles((prev) =>
              prev.map((p) => (p.id === accountId ? { ...p, skinUrl: skinSrc } : p))
            );
          } catch {
            // 重新下载也失败，静默忽略
          }
        }
      })();
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
        yggdrasilUrl: "https://littleskin.cn/api/yggdrasil",
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
        const accountId = `ls-${info.uuid}`;
        const skinUrlKey = info.skin_url;
        const accountUuid = info.uuid;
        (async () => {
          try {
            const skinSrc = await getSkinBase64(skinUrlKey);
            setProfiles((prev) =>
              prev.map((p) => (p.id === accountId ? { ...p, skinUrl: skinSrc } : p))
            );
          } catch {
            // 第一次失败：尝试重新下载皮肤
            try {
              await redownloadLittleSkinSkin(accountUuid);
              const skinSrc = await getSkinBase64(skinUrlKey);
              setProfiles((prev) =>
                prev.map((p) => (p.id === accountId ? { ...p, skinUrl: skinSrc } : p))
              );
            } catch {
              // 重新下载也失败，静默忽略
            }
          }
        })();
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
    // 清理强制登录 DeviceCode + 强制模式
    setForcedMicrosoftDeviceCode(null);
    setForcedMicrosoftLoginMode(false);
    setForcedMicrosoftLoginMessage(null);
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
        updateProfile,
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
        forcedMicrosoftDeviceCode,
        forcedMicrosoftLoginMode,
        forcedMicrosoftLoginMessage,
        clearForcedMicrosoftDeviceCode,
        manualLoginOpen,
        setManualLoginOpen,
      }}
    >
      {children}
      {/*
        🔴 关键修复：LoginDialog 直接放在 AccountProvider 内全局渲染，
        不再依赖 AccountSwitcher 是否被用户打开过！
        - forcedMicrosoftLoginMode=true → 强制登录模式（必须是 microsoft tab，不可关闭）
        - manualLoginOpen=true         → 用户点了"添加账户"按钮的普通模式
        两者只要有一个是 true，就显示 LoginDialog。
      */}
      <LoginDialog
        open={forcedMicrosoftLoginMode || manualLoginOpen}
        onClose={() => {
          // 强制登录模式下忽略 onClose（LoginDialog 内部也做了防护）
          // 普通模式下：关闭手动登录弹窗
          setManualLoginOpen(false);
        }}
        forcedDeviceCode={forcedMicrosoftDeviceCode}
        onForcedDeviceCodeConsumed={clearForcedMicrosoftDeviceCode}
        forcedMode={forcedMicrosoftLoginMode}
        forcedMessage={forcedMicrosoftLoginMessage}
      />
    </AccountContext.Provider>
  );
}

export function useAccountContext() {
  const ctx = useContext(AccountContext);
  if (!ctx)
    throw new Error("useAccountContext must be used within AccountProvider");
  return ctx;
}