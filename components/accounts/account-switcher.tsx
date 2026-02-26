"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { useAccountContext } from "@/components/accounts/account-provider";
import { X, Check } from "lucide-react";
import { cn } from "@/lib/utils";
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
  const { profiles, selectedProfile } = useAccountContext();

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* 遮罩 */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* 弹窗内容 */}
      <Card className="relative z-10 w-full max-w-md mx-4 shadow-2xl">
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>切换账户</CardTitle>
          <Button variant="ghost" size="icon-sm" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </CardHeader>
        <CardContent className="space-y-2">
          {profiles.map((profile) => (
            <button
              key={profile.id}
              type="button"
              className={cn(
                "flex w-full items-center gap-3 rounded-xl p-3 text-left transition-colors hover:bg-accent",
                selectedProfile?.id === profile.id && "bg-accent"
              )}
              onClick={() => {
                onSelect(profile);
                onClose();
              }}
            >
              <Avatar>
                <AvatarFallback>
                  {profile.name.charAt(0).toUpperCase()}
                </AvatarFallback>
              </Avatar>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium truncate">{profile.name}</p>
                <p className="text-xs text-muted-foreground">
                  {profile.status}
                </p>
              </div>
              {selectedProfile?.id === profile.id && (
                <Check className="size-4 text-primary" />
              )}
            </button>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
