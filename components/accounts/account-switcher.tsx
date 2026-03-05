"use client";

import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { useAccountContext } from "@/components/accounts/account-provider";
import { LoginDialog } from "@/components/accounts/login-dialog";
import { X, Check, Plus, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { overlayFade, scaleIn } from "@/lib/motion";
import type { Account } from "@/types";

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
  const { profiles, selectedProfile, removeProfile } = useAccountContext();
  const [loginOpen, setLoginOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Account | null>(null);

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
                  <CardTitle>切换账户</CardTitle>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      onClick={() => setLoginOpen(true)}
                      title="添加账户"
                    >
                      <Plus className="size-4" />
                    </Button>
                    <Button variant="ghost" size="icon-sm" onClick={onClose}>
                      <X className="size-4" />
                    </Button>
                  </div>
                </CardHeader>
                <CardContent className="space-y-2">
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
                          {profile.skinUrl && (
                            <AvatarImage src={profile.skinUrl} alt={profile.name} />
                          )}
                          <AvatarFallback>
                            {profile.name.charAt(0).toUpperCase()}
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
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        className="opacity-0 group-hover:opacity-100 transition-opacity"
                        onClick={(e) => {
                          e.stopPropagation();
                          setDeleteTarget(profile);
                        }}
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

      {/* 登录对话框 */}
      <LoginDialog open={loginOpen} onClose={() => setLoginOpen(false)} />
    </>
  );
}
