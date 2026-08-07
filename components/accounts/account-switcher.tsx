"use client";

import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { useAccountContext } from "@/components/accounts/account-provider";
import { SkinCapeManager } from "@/components/accounts/skin-cape-manager";
import { X, Check, Plus, Trash2, Shirt, RefreshCw } from "lucide-react";
import { cn, getAvatarColor, getAvatarInitials } from "@/lib/utils";
import { overlayFade, scaleIn } from "@/lib/motion";
import type { Account } from "@/types";
import { redownloadLittleSkinSkin, getSkinBase64 } from "@/lib/auth";

interface AccountSwitcherProps {
  open: boolean;
  onClose: () => void;
  onSelect: (account: Account) => void;
}

export function AccountSwitcher({
  open,
  onClose,
  onSelect,
}: AccountSwitcherProps) {
  const {
    profiles,
    selectedProfile,
    removeProfile,
    updateProfile,
    setManualLoginOpen,
  } = useAccountContext();
  const [deleteTarget, setDeleteTarget] = useState<Account | null>(null);
  const [skinManagerAccount, setSkinManagerAccount] = useState<Account | null>(null);
  const [refreshingSkinId, setRefreshingSkinId] = useState<string | null>(null);

  return (
    <>
      <AnimatePresence>
        {open && (
          <div className="fixed inset-0 z-50 flex items-center justify-center">
            {/* 遮罩 */}
            <motion.div
              variants={overlayFade}
              initial="initial"
              animate="animate"
              exit="exit"
              className="absolute inset-0 bg-black/50 backdrop-blur-sm"
              onClick={onClose}
            />

            {/* 弹窗内容 */}
            <motion.div
              variants={scaleIn}
              initial="initial"
              animate="animate"
              exit="exit"
              className="relative z-10 w-full max-w-md mx-4"
            >
              <Card className="shadow-2xl">
                <CardHeader className="flex flex-row items-center justify-between">
                  <CardTitle>管理账户</CardTitle>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      className="touch-manipulation"
                      onClick={() => setManualLoginOpen(true)}
                      title="添加账户"
                    >
                      <Plus className="size-4" />
                    </Button>
                    <Button variant="ghost" size="icon-sm" className="touch-manipulation" onClick={onClose}>
                      <X className="size-4" />
                    </Button>
                  </div>
                </CardHeader>
                <CardContent className="space-y-2 max-h-[65vh] overflow-y-auto">
                  {profiles.length === 0 && (
                    <p className="text-center text-sm text-muted-foreground py-4">
                      暂无账户，请点击右上角 + 添加
                    </p>
                  )}
                  {profiles.map((profile) => (
                    <div
                      key={profile.id}
                      className={cn(
                        "flex w-full items-center gap-3 rounded-xl p-3 transition-colors hover:bg-accent group",
                        selectedProfile?.id === profile.id && "bg-accent"
                      )}
                    >
                      <button
                        type="button"
                        className="flex flex-1 items-center gap-3 text-left"
                        onClick={() => {
                          onSelect(profile);
                          onClose();
                        }}
                      >
                        <Avatar>
                          <AvatarFallback
                            className={cn(
                              getAvatarColor(profile.name),
                              "text-white font-medium"
                            )}
                          >
                            {getAvatarInitials(profile.name)}
                          </AvatarFallback>
                        </Avatar>
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-medium truncate">
                            {profile.name}
                          </p>
                          <p className="text-xs text-muted-foreground">
                            {profile.status}
                          </p>
                        </div>
                        {selectedProfile?.id === profile.id && (
                          <Check className="size-4 text-primary" />
                        )}
                      </button>

                      {/* 微软正版账户：显示皮肤/披风管理按钮 */}
                      {profile.authType === "microsoft" && (
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          className="opacity-0 group-hover:opacity-100 transition-opacity touch-manipulation"
                          onClick={(e) => {
                            e.stopPropagation();
                            setSkinManagerAccount(profile);
                          }}
                          title="皮肤与披风管理"
                        >
                          <Shirt className="size-4" />
                        </Button>
                      )}

                      {/* LittleSkin 账户：显示刷新皮肤按钮 */}
                      {profile.authType === "littleskin" && (
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          disabled={refreshingSkinId === profile.id}
                          className="opacity-0 group-hover:opacity-100 transition-opacity touch-manipulation disabled:opacity-100"
                          onClick={(e) => {
                            e.stopPropagation();
                            if (!profile.uuid || refreshingSkinId) return;
                            const pid = profile.id;
                            const puuid = profile.uuid;
                            setRefreshingSkinId(pid);
                            // 用 IIFE 触发真正异步，避免 onClick 返回 Promise 导致 React 传播 Event 错误
                            (async () => {
                              try {
                                await redownloadLittleSkinSkin(puuid);
                                const skinSrc = await getSkinBase64(puuid);
                                updateProfile(pid, { skinUrl: skinSrc });
                              } catch (err) {
                                console.warn("刷新皮肤失败:", err);
                              } finally {
                                setRefreshingSkinId((curr) => (curr === pid ? null : curr));
                              }
                            })();
                          }}
                          title="刷新皮肤"
                        >
                          <RefreshCw
                            className={`size-4 ${
                              refreshingSkinId === profile.id ? "animate-spin" : ""
                            }`}
                          />
                        </Button>
                      )}

                      <Button
                        variant="ghost"
                        size="icon-sm"
                        className="opacity-0 group-hover:opacity-100 transition-opacity touch-manipulation"
                        onClick={(e) => {
                          e.stopPropagation();
                          setDeleteTarget(profile);
                        }}
                        title="删除账户"
                      >
                        <Trash2 className="size-3.5 text-muted-foreground" />
                      </Button>
                    </div>
                  ))}
                </CardContent>
              </Card>
            </motion.div>
          </div>
        )}
      </AnimatePresence>

      {/* 删除确认弹窗 */}
      <AnimatePresence>
        {deleteTarget && (
          <div className="fixed inset-0 z-[60] flex items-center justify-center">
            <motion.div
              variants={overlayFade}
              initial="initial"
              animate="animate"
              exit="exit"
              className="absolute inset-0 bg-black/50 backdrop-blur-sm"
              onClick={() => setDeleteTarget(null)}
            />
            <motion.div
              variants={scaleIn}
              initial="initial"
              animate="animate"
              exit="exit"
              className="relative z-10 w-full max-w-xs mx-4"
            >
              <Card className="shadow-2xl">
                <CardHeader>
                  <CardTitle className="text-base">确认删除</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <p className="text-sm text-muted-foreground">
                    确定要删除账户 <span className="font-medium text-foreground">{deleteTarget.name}</span> 吗？此操作不可撤销。
                  </p>
                  <div className="flex gap-2 justify-end">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setDeleteTarget(null)}
                    >
                      取消
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      onClick={() => {
                        removeProfile(deleteTarget.id);
                        setDeleteTarget(null);
                      }}
                    >
                      删除
                    </Button>
                  </div>
                </CardContent>
              </Card>
            </motion.div>
          </div>
        )}
      </AnimatePresence>

      {/* 皮肤/披风管理弹窗 */}
      {skinManagerAccount && (
        <SkinCapeManager
          account={skinManagerAccount}
          onClose={() => setSkinManagerAccount(null)}
        />
      )}
    </>
  );
}