"use client";

import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useAccountContext } from "@/components/accounts/account-provider";
import { X, Globe, User, Loader2, Shield, Check, Copy } from "lucide-react";
import { cn } from "@/lib/utils";
import { overlayFade, scaleIn, fadeSlideUp } from "@/lib/motion";
import type { ThirdPartyProfile } from "@/lib/auth";
import type { DeviceCodeInfo } from "@/lib/auth";

type LoginTab = "microsoft" | "littleskin" | "third_party" | "offline";

interface LoginDialogProps {
  open: boolean;
  onClose: () => void;
}

export function LoginDialog({ open, onClose }: LoginDialogProps) {
  const {
    loginWithLittleSkin,
    loginWithThirdParty,
    addOfflineAccount,
    addThirdPartyAccount,
    loginWithMicrosoft,
    loginState,
    loginError,
  } = useAccountContext();

  const [tab, setTab] = useState<LoginTab>("microsoft");

  // 第三方登录表单
  const [tpUrl, setTpUrl] = useState("");
  const [tpUser, setTpUser] = useState("");
  const [tpPwd, setTpPwd] = useState("");
  const [tpProfiles, setTpProfiles] = useState<ThirdPartyProfile[] | null>(null);
  const [tpAccessToken, setTpAccessToken] = useState("");

  // 离线登录
  const [offlineName, setOfflineName] = useState("");

  // 微软正版登录
  const [msDeviceCode, setMsDeviceCode] = useState<DeviceCodeInfo | null>(null);
  const [msPolling, setMsPolling] = useState(false);
  const [codeCopied, setCodeCopied] = useState(false);

  // 微软登录轮询完成时自动关闭弹窗
  useEffect(() => {
    if (msPolling && loginState === "idle" && !loginError) {
      setMsPolling(false);
      setMsDeviceCode(null);
      setCodeCopied(false);
      onClose();
    }
  }, [msPolling, loginState, loginError, onClose]);

  const isLoading = loginState === "loading";

  // Tab content key changes when tab switches or sub-state changes
  const tabContentKey = `${tab}-${msDeviceCode ? "device" : ""}-${tpProfiles ? "profiles" : ""}`;

  const handleLittleSkin = async () => {
    await loginWithLittleSkin();
    if (loginState !== "error") onClose();
  };

  const handleThirdPartyLogin = async () => {
    try {
      const result = await loginWithThirdParty(tpUrl, tpUser, tpPwd);
      if (result.profiles.length === 1) {
        // 只有一个角色，直接添加
        addThirdPartyAccount(result.profiles[0], result.access_token, tpUrl);
        onClose();
      } else {
        // 多个角色，让用户选择
        setTpProfiles(result.profiles);
        setTpAccessToken(result.access_token);
      }
    } catch {
      // error 已由 provider 处理
    }
  };

  const handleSelectProfile = (profile: ThirdPartyProfile) => {
    addThirdPartyAccount(profile, tpAccessToken, tpUrl);
    onClose();
  };

  const handleOffline = () => {
    if (!offlineName.trim()) return;
    addOfflineAccount(offlineName.trim());
    onClose();
  };

  const handleMicrosoft = async () => {
    try {
      const codeInfo = await loginWithMicrosoft();
      setMsDeviceCode(codeInfo);
      setMsPolling(true);
      // 浏览器由 Rust 后端自动打开
      // 自动复制验证码到剪贴板
      try {
        await navigator.clipboard.writeText(codeInfo.user_code);
        setCodeCopied(true);
      } catch {
        // 剪贴板不可用时静默失败
      }
    } catch {
      // error 已由 provider 处理
    }
  };

  const tabs: { id: LoginTab; label: string; icon: React.ReactNode }[] = [
    { id: "microsoft", label: "正版登录", icon: <Shield className="size-4" /> },
    { id: "littleskin", label: "LittleSkin", icon: <Globe className="size-4" /> },
    { id: "third_party", label: "第三方登录", icon: <Globe className="size-4" /> },
    { id: "offline", label: "离线登录", icon: <User className="size-4" /> },
  ];

  return (
    <AnimatePresence>
      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center">
          <motion.div
            variants={overlayFade}
            initial="initial"
            animate="animate"
            exit="exit"
            className="absolute inset-0 bg-black/50 backdrop-blur-sm"
            onClick={onClose}
          />

          <motion.div
            variants={scaleIn}
            initial="initial"
            animate="animate"
            exit="exit"
            className="relative z-10 w-full max-w-md mx-4"
          >
            <Card className="shadow-2xl">
              <CardHeader className="flex flex-row items-center justify-between">
                <CardTitle>添加账户</CardTitle>
                <Button variant="ghost" size="icon-sm" onClick={onClose}>
                  <X className="size-4" />
                </Button>
              </CardHeader>

              <CardContent className="space-y-4">
                {/* Tab 切换 */}
                <div className="grid grid-cols-4 gap-1 p-1 rounded-lg bg-muted">
                  {tabs.map((t) => (
                    <button
                      key={t.id}
                      type="button"
                      className={cn(
                        "flex items-center justify-center gap-1 rounded-md px-2 py-1.5 text-xs font-medium transition-colors whitespace-nowrap",
                        tab === t.id
                          ? "bg-background text-foreground shadow-sm"
                          : "text-muted-foreground hover:text-foreground"
                      )}
                      onClick={() => {
                        setTab(t.id);
                        setTpProfiles(null);
                        setCodeCopied(false);
                      }}
                    >
                      {t.icon}
                      {t.label}
                    </button>
                  ))}
                </div>

                {/* Tab 内容区域（带动画切换） */}
                <AnimatePresence mode="wait">
                  <motion.div
                    key={tabContentKey}
                    variants={fadeSlideUp}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    className="space-y-4"
                  >
                    {/* 错误展示 */}
                    {loginError && (
                      <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                        {loginError}
                      </div>
                    )}

                    {/* 微软正版登录 */}
                    {tab === "microsoft" && !msDeviceCode && (
                      <div className="space-y-4">
                        <p className="text-sm text-muted-foreground">
                          使用微软账户进行正版登录，点击下方按钮获取验证码。
                        </p>
                        <Button
                          className="w-full"
                          onClick={handleMicrosoft}
                          disabled={isLoading}
                        >
                          {isLoading ? (
                            <>
                              <Loader2 className="size-4 mr-2 animate-spin" />
                              获取验证码中…
                            </>
                          ) : (
                            "使用微软账户登录"
                          )}
                        </Button>
                      </div>
                    )}

                    {/* 微软正版登录 - 显示验证码 */}
                    {tab === "microsoft" && msDeviceCode && (
                      <div className="space-y-4">
                        <p className="text-sm text-muted-foreground">
                          请在浏览器中访问以下网址，并输入验证码完成授权：
                        </p>
                        <div className="rounded-lg border bg-muted/50 p-4 text-center space-y-3">
                          <p className="text-xs text-muted-foreground">访问网址</p>
                          <a
                            href={msDeviceCode.verification_uri}
                            target="_blank"
                            rel="noreferrer"
                            className="text-sm font-medium text-primary underline underline-offset-4 hover:text-primary/80"
                          >
                            {msDeviceCode.verification_uri}
                          </a>
                          <p className="text-xs text-muted-foreground mt-3">输入验证码</p>
                          <div className="flex items-center justify-center gap-2">
                            <p className="text-2xl font-bold tracking-widest font-mono">
                              {msDeviceCode.user_code}
                            </p>
                            <button
                              type="button"
                              onClick={() => {
                                navigator.clipboard.writeText(msDeviceCode.user_code).then(() => setCodeCopied(true));
                              }}
                              className="text-muted-foreground hover:text-foreground transition-colors"
                              title="复制验证码"
                            >
                              {codeCopied ? <Check className="size-4 text-green-500" /> : <Copy className="size-4" />}
                            </button>
                          </div>
                          {codeCopied && (
                            <p className="text-xs text-green-500">已复制到剪贴板</p>
                          )}
                        </div>
                        <div className="flex items-center justify-center gap-2 text-sm text-muted-foreground">
                          <Loader2 className="size-4 animate-spin" />
                          等待授权完成…
                        </div>
                      </div>
                    )}

                    {/* LittleSkin 登录 */}
                    {tab === "littleskin" && (
                      <div className="space-y-4">
                        <p className="text-sm text-muted-foreground">
                          点击下方按钮将打开浏览器，通过 LittleSkin OAuth 进行授权登录。
                        </p>
                        <Button
                          className="w-full"
                          onClick={handleLittleSkin}
                          disabled={isLoading}
                        >
                          {isLoading ? (
                            <>
                              <Loader2 className="size-4 mr-2 animate-spin" />
                              等待授权中…
                            </>
                          ) : (
                            "使用 LittleSkin 登录"
                          )}
                        </Button>
                      </div>
                    )}

                    {/* 第三方 Yggdrasil 登录 */}
                    {tab === "third_party" && !tpProfiles && (
                      <div className="space-y-3">
                        <div className="space-y-1.5">
                          <Label htmlFor="tp-url">认证服务器地址</Label>
                          <Input
                            id="tp-url"
                            placeholder="https://example.com/api/yggdrasil"
                            value={tpUrl}
                            onChange={(e) => setTpUrl(e.target.value)}
                            disabled={isLoading}
                          />
                        </div>
                        <div className="space-y-1.5">
                          <Label htmlFor="tp-user">用户名 / 邮箱</Label>
                          <Input
                            id="tp-user"
                            placeholder="user@example.com"
                            value={tpUser}
                            onChange={(e) => setTpUser(e.target.value)}
                            disabled={isLoading}
                          />
                        </div>
                        <div className="space-y-1.5">
                          <Label htmlFor="tp-pwd">密码</Label>
                          <Input
                            id="tp-pwd"
                            type="password"
                            value={tpPwd}
                            onChange={(e) => setTpPwd(e.target.value)}
                            disabled={isLoading}
                          />
                        </div>
                        <Button
                          className="w-full"
                          onClick={handleThirdPartyLogin}
                          disabled={isLoading || !tpUrl || !tpUser || !tpPwd}
                        >
                          {isLoading ? (
                            <>
                              <Loader2 className="size-4 mr-2 animate-spin" />
                              登录中…
                            </>
                          ) : (
                            "登录"
                          )}
                        </Button>
                      </div>
                    )}

                    {/* 第三方登录 - 选择角色 */}
                    {tab === "third_party" && tpProfiles && (
                      <div className="space-y-3">
                        <p className="text-sm text-muted-foreground">
                          请选择要使用的游戏角色：
                        </p>
                        {tpProfiles.map((profile) => (
                          <button
                            key={profile.id}
                            type="button"
                            className="flex w-full items-center gap-3 rounded-xl p-3 text-left transition-colors hover:bg-accent"
                            onClick={() => handleSelectProfile(profile)}
                          >
                            <div className="size-8 rounded-full bg-primary/10 flex items-center justify-center">
                              <User className="size-4" />
                            </div>
                            <div>
                              <p className="text-sm font-medium">{profile.name}</p>
                              <p className="text-xs text-muted-foreground font-mono">
                                {profile.id}
                              </p>
                            </div>
                          </button>
                        ))}
                        <Button
                          variant="ghost"
                          className="w-full"
                          onClick={() => setTpProfiles(null)}
                        >
                          返回
                        </Button>
                      </div>
                    )}

                    {/* 离线登录 */}
                    {tab === "offline" && (
                      <div className="space-y-3">
                        <div className="space-y-1.5">
                          <Label htmlFor="offline-name">游戏名称</Label>
                          <Input
                            id="offline-name"
                            placeholder="Steve"
                            value={offlineName}
                            onChange={(e) => setOfflineName(e.target.value)}
                          />
                        </div>
                        <Button
                          className="w-full"
                          onClick={handleOffline}
                          disabled={!offlineName.trim()}
                        >
                          离线登录
                        </Button>
                      </div>
                    )}
                  </motion.div>
                </AnimatePresence>
              </CardContent>
            </Card>
          </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
