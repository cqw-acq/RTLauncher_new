"use client";

import { useEffect, useState, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  msGetSkinsAndCapes,
  msUploadSkin,
  msActivateSkin,
  msDeleteSkin,
  msSetActiveCape,
  microsoftProbeAccountLogin,
  type MCSkinInfo,
  type MCCapeInfo,
} from "@/lib/auth";
import { X, Check, Upload, Trash2, RefreshCw, Shirt, Sparkles } from "lucide-react";
import { overlayFade, scaleIn } from "@/lib/motion";
import type { Account } from "@/types";
import { cn } from "@/lib/utils";

interface SkinCapeManagerProps {
  account: Account;
  onClose: () => void;
}

type TabType = "skins" | "capes";

export function SkinCapeManager({ account, onClose }: SkinCapeManagerProps) {
  const [tab, setTab] = useState<TabType>("skins");
  const [loading, setLoading] = useState(true);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [skins, setSkins] = useState<MCSkinInfo[]>([]);
  const [capes, setCapes] = useState<MCCapeInfo[]>([]);
  const [uploadPreview, setUploadPreview] = useState<string | null>(null);
  const [uploadVariant, setUploadVariant] = useState<"classic" | "slim">("classic");
  const [busyAction, setBusyAction] = useState<string | null>(null); // 正在执行的操作 ID

  const fileInputRef = useRef<HTMLInputElement>(null);

  // 加载皮肤/披风列表 —— 调用【公共函数 microsoftProbeAccountLogin】
  // 保证和 AccountProvider 启动时的账号有效性检测逻辑 100% 一致
  const loadProfile = async () => {
    setLoading(true);
    setErrorMsg(null);
    const probe = await microsoftProbeAccountLogin(account.accessToken);
    if (probe.ok) {
      setSkins(probe.profile.skins || []);
      setCapes(probe.profile.capes || []);
      setLoading(false);
    } else {
      setErrorMsg(probe.error); // 错误消息原封不动返回（和启动时检测的错误消息完全一样）
      setLoading(false);
    }
  };

  useEffect(() => {
    loadProfile();
  }, [account.accessToken]);

  // 处理文件选择 -> 预览并转 base64
  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!file.name.toLowerCase().endsWith(".png")) {
      setErrorMsg("仅支持 PNG 格式的皮肤文件");
      return;
    }
    const reader = new FileReader();
    reader.onload = (ev) => {
      const result = ev.target?.result;
      if (typeof result === "string") {
        setUploadPreview(result);
      }
    };
    reader.readAsDataURL(file);
  };

  // 上传皮肤
  const handleUpload = async () => {
    if (!uploadPreview || !account.accessToken) return;
    // 剥离 data:image/png;base64, 前缀，只传纯 base64
    const base64Only = uploadPreview.replace(/^data:image\/\w+;base64,/, "");
    setBusyAction("upload");
    setErrorMsg(null);
    try {
      await msUploadSkin(account.accessToken, base64Only, uploadVariant);
      setUploadPreview(null);
      if (fileInputRef.current) fileInputRef.current.value = "";
      // 上传成功后刷新列表
      await loadProfile();
    } catch (e: unknown) {
      setErrorMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyAction(null);
    }
  };

  // 激活指定皮肤
  const handleActivateSkin = async (skinId: string, variant: string) => {
    if (!account.accessToken) return;
    setBusyAction(`activate-${skinId}`);
    setErrorMsg(null);
    try {
      await msActivateSkin(account.accessToken, skinId, variant as "classic" | "slim");
      await loadProfile();
    } catch (e: unknown) {
      setErrorMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyAction(null);
    }
  };

  // 删除皮肤
  const handleDeleteSkin = async (skinId: string) => {
    if (!account.accessToken) return;
    if (!confirm("确认删除此皮肤？删除后无法恢复")) return;
    setBusyAction(`delete-${skinId}`);
    setErrorMsg(null);
    try {
      await msDeleteSkin(account.accessToken, skinId);
      await loadProfile();
    } catch (e: unknown) {
      setErrorMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyAction(null);
    }
  };

  // 设置激活披风
  const handleSetCape = async (capeId: string) => {
    if (!account.accessToken) return;
    setBusyAction(`cape-${capeId}`);
    setErrorMsg(null);
    try {
      await msSetActiveCape(account.accessToken, capeId);
      await loadProfile();
    } catch (e: unknown) {
      setErrorMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyAction(null);
    }
  };

  // 取消激活披风
  const handleDeactivateCape = async () => {
    if (!account.accessToken) return;
    setBusyAction("cape-deactivate");
    setErrorMsg(null);
    try {
      await msSetActiveCape(account.accessToken, "");
      await loadProfile();
    } catch (e: unknown) {
      setErrorMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyAction(null);
    }
  };

  const activeSkinId = skins.find((s) => s.state === "ACTIVE")?.id;
  const activeCapeId = capes.find((c) => c.state === "ACTIVE")?.id;

  return (
    <AnimatePresence>
      <div className="fixed inset-0 z-[70] flex items-center justify-center">
        <motion.div
          variants={overlayFade}
          initial="initial"
          animate="animate"
          exit="exit"
          className="absolute inset-0 bg-black/60 backdrop-blur-sm"
          onClick={onClose}
        />
        <motion.div
          variants={scaleIn}
          initial="initial"
          animate="animate"
          exit="exit"
          className="relative z-10 w-full max-w-3xl mx-4"
        >
          <Card className="shadow-2xl">
            <CardHeader className="flex flex-row items-center justify-between">
              <div className="flex items-center gap-3">
                <Shirt className="size-5 text-primary" />
                <CardTitle className="text-lg">
                  皮肤与披风 — {account.name}
                </CardTitle>
              </div>
              <div className="flex items-center gap-1">
                <Button
                  variant="ghost"
                  size="icon-sm"
                  onClick={loadProfile}
                  disabled={loading}
                  title="刷新"
                >
                  <RefreshCw className={cn("size-4", loading && "animate-spin")} />
                </Button>
                <Button variant="ghost" size="icon-sm" onClick={onClose}>
                  <X className="size-4" />
                </Button>
              </div>
            </CardHeader>

            <CardContent className="space-y-4">
              {/* Tab 切换 */}
              <div className="flex gap-2 border-b">
                <button
                  type="button"
                  onClick={() => setTab("skins")}
                  className={cn(
                    "px-4 py-2 text-sm font-medium border-b-2 transition-colors",
                    tab === "skins"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  )}
                >
                  皮肤 ({skins.length})
                </button>
                <button
                  type="button"
                  onClick={() => setTab("capes")}
                  className={cn(
                    "px-4 py-2 text-sm font-medium border-b-2 transition-colors",
                    tab === "capes"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  )}
                >
                  披风 ({capes.length})
                </button>
              </div>

              {/* 错误信息 */}
              {errorMsg && (
                <div className="text-sm text-destructive bg-destructive/10 rounded-md px-3 py-2">
                  {errorMsg}
                </div>
              )}

              {/* 加载中 */}
              {loading && (
                <div className="text-center text-sm text-muted-foreground py-8">
                  正在加载皮肤与披风信息...
                </div>
              )}

              {/* 皮肤 Tab */}
              {!loading && tab === "skins" && (
                <div className="space-y-4">
                  {/* 上传皮肤 */}
                  <div className="border-2 border-dashed rounded-lg p-4 bg-muted/30">
                    <div className="flex flex-col sm:flex-row items-center gap-4">
                      <div className="flex-1 w-full">
                        <input
                          ref={fileInputRef}
                          type="file"
                          accept="image/png"
                          className="hidden"
                          onChange={handleFileChange}
                        />
                        <div className="flex items-center gap-3">
                          {uploadPreview ? (
                            <img
                              src={uploadPreview}
                              alt="皮肤预览"
                              className="size-16 rounded-md bg-muted object-contain border shadow-sm"
                            />
                          ) : (
                            <div className="size-16 rounded-md bg-muted flex items-center justify-center border">
                              <Sparkles className="size-6 text-muted-foreground" />
                            </div>
                          )}
                          <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium">上传新皮肤</p>
                            <p className="text-xs text-muted-foreground">
                              选择 64x64 或 64x32 的 PNG 皮肤图片
                            </p>
                            <div className="flex flex-wrap items-center gap-2 mt-2">
                              <Button
                                size="sm"
                                onClick={() => fileInputRef.current?.click()}
                              >
                                <Upload className="size-4 mr-1" />
                                选择文件
                              </Button>
                              {uploadPreview && (
                                <>
                                  <div className="flex items-center gap-2 text-xs">
                                    <label className="flex items-center gap-1 cursor-pointer">
                                      <input
                                        type="radio"
                                        name="skin-variant"
                                        value="classic"
                                        checked={uploadVariant === "classic"}
                                        onChange={() => setUploadVariant("classic")}
                                      />
                                      <span>经典</span>
                                    </label>
                                    <label className="flex items-center gap-1 cursor-pointer">
                                      <input
                                        type="radio"
                                        name="skin-variant"
                                        value="slim"
                                        checked={uploadVariant === "slim"}
                                        onChange={() => setUploadVariant("slim")}
                                      />
                                      <span>细胳膊</span>
                                    </label>
                                  </div>
                                  <Button
                                    size="sm"
                                    variant="default"
                                    onClick={handleUpload}
                                    disabled={busyAction !== null}
                                  >
                                    {busyAction === "upload" ? (
                                      <RefreshCw className="size-4 mr-1 animate-spin" />
                                    ) : (
                                      <Check className="size-4 mr-1" />
                                    )}
                                    上传并使用
                                  </Button>
                                </>
                              )}
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* 皮肤列表 */}
                  {skins.length === 0 ? (
                    <p className="text-center text-sm text-muted-foreground py-6">
                      该账户暂无皮肤，请在上方上传或前往 minecraft.net 管理
                    </p>
                  ) : (
                    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
                      {skins.map((skin) => {
                        const isActive = skin.id === activeSkinId;
                        const isBusy = busyAction === `activate-${skin.id}` || busyAction === `delete-${skin.id}`;
                        return (
                          <div
                            key={skin.id}
                            className={cn(
                              "rounded-lg border p-3 flex flex-col items-center gap-2 transition-all",
                              isActive && "border-primary bg-primary/5 shadow-sm"
                            )}
                          >
                            <img
                              src={skin.url}
                              alt={skin.alias || "皮肤"}
                              className="size-20 rounded-md bg-muted object-contain shadow-sm"
                            />
                            <div className="text-xs text-center space-y-0.5 w-full">
                              <p className="font-medium truncate">
                                {skin.alias || "未命名"}
                              </p>
                              <p className="text-muted-foreground">
                                {skin.variant}
                              </p>
                              {isActive && (
                                <p className="text-primary text-[10px] font-semibold">
                                  ● 当前使用
                                </p>
                              )}
                            </div>
                            <div className="flex gap-1 w-full">
                              {!isActive && (
                                <Button
                                  size="sm"
                                  variant="outline"
                                  className="flex-1 text-xs"
                                  onClick={() => handleActivateSkin(skin.id, skin.variant)}
                                  disabled={busyAction !== null}
                                >
                                  {isBusy ? (
                                    <RefreshCw className="size-3 animate-spin" />
                                  ) : (
                                    "使用"
                                  )}
                                </Button>
                              )}
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => handleDeleteSkin(skin.id)}
                                disabled={busyAction !== null}
                                title="删除此皮肤"
                              >
                                <Trash2 className="size-3.5 text-muted-foreground hover:text-destructive" />
                              </Button>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              )}

              {/* 披风 Tab */}
              {!loading && tab === "capes" && (
                <div className="space-y-4">
                  {/* 取消激活按钮 */}
                  {activeCapeId && (
                    <div className="flex items-center justify-between border rounded-lg p-3 bg-muted/30">
                      <div className="text-sm">
                        当前已装备披风：
                        <span className="font-medium text-primary">
                          {capes.find((c) => c.id === activeCapeId)?.alias || "未命名披风"}
                        </span>
                      </div>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={handleDeactivateCape}
                        disabled={busyAction !== null}
                      >
                        {busyAction === "cape-deactivate" ? (
                          <RefreshCw className="size-3.5 mr-1 animate-spin" />
                        ) : null}
                        卸下披风
                      </Button>
                    </div>
                  )}

                  {capes.length === 0 ? (
                    <p className="text-center text-sm text-muted-foreground py-6">
                      该账户暂无可用披风
                    </p>
                  ) : (
                    <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
                      {capes.map((cape) => {
                        const isActive = cape.id === activeCapeId;
                        const isBusy = busyAction === `cape-${cape.id}`;
                        return (
                          <div
                            key={cape.id}
                            className={cn(
                              "rounded-lg border p-3 flex flex-col items-center gap-2 transition-all cursor-pointer hover:shadow-md",
                              isActive && "border-primary bg-primary/5 shadow-sm"
                            )}
                            onClick={() => handleSetCape(cape.id)}
                          >
                            <img
                              src={cape.url}
                              alt={cape.alias || "披风"}
                              className="size-20 rounded-md bg-muted object-contain shadow-sm"
                            />
                            <div className="text-xs text-center space-y-0.5 w-full">
                              <p className="font-medium truncate">
                                {cape.alias || "未命名"}
                              </p>
                              {isActive && (
                                <p className="text-primary text-[10px] font-semibold">
                                  ● 已装备
                                </p>
                              )}
                              {isBusy && !isActive && (
                                <p className="text-muted-foreground text-[10px]">
                                  切换中...
                                </p>
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              )}
            </CardContent>
          </Card>
        </motion.div>
      </div>
    </AnimatePresence>
  );
}