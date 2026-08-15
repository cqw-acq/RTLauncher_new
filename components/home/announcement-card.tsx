"use client";

import { AnimatePresence, motion } from "framer-motion";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ANNOUNCEMENTS } from "@/constants/data";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Announcement } from "@/types";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { slideLeftContent } from "@/lib/motion";
import { useI18n } from "@/components/i18n/use-i18n";

const ANNOUNCEMENT_COPY = [
  { title: "home.announcement.welcomeToRtlauncher", content: "home.announcement.aModernMinecraftLauncherWithAPolishedSmoothExperience" },
  { title: "home.announcement.systemUpdate", content: "home.announcement.weRecentlyReleasedNewFeaturesAndImprovedTheExperience" },
  { title: "home.announcement.tips", content: "home.announcement.readTheDocumentationToGetTheMostFromThe" },
] as const;

export function AnnouncementCard({ compact = false }: { compact?: boolean }) {
  const { t, language } = useI18n();
  const [current, setCurrent] = useState(0);
  const [remote, setRemote] = useState<Announcement[] | null>(null);
  const announcement = (remote && remote.length > 0 ? remote : ANNOUNCEMENT_COPY)[current] ?? ANNOUNCEMENT_COPY[0];

  useEffect(() => {
    // 拉取本地 announcements 文件（由后端定期 sync 到仓库）
    const langCode = language === "zh-CN" ? "zh_cn" : "en_us";
    void invoke<Announcement[]>("get_announcements", { language: langCode })
      .then((list) => {
        if (list && list.length > 0) setRemote(list);
      })
      .catch(() => {});
  }, [language]);

  const total = (remote && remote.length > 0) ? remote.length : ANNOUNCEMENT_COPY.length;
  const prev = () =>
    setCurrent((i) => (i - 1 + total) % total);
  const next = () =>
    setCurrent((i) => (i + 1) % total);

  // 当远程公告存在时直接显示其文本；否则对内置副本使用 i18n 翻译键
  const displayedTitle = remote && remote.length > 0 ? announcement.title : t((announcement as any).title);
  const displayedContent = remote && remote.length > 0 ? announcement.content : t((announcement as any).content);

  if (compact) {
    return (
      <Card className="w-full h-full shadow-sm flex flex-col">
        <CardHeader className="pb-2 px-3">
          <CardTitle className="text-sm">{t("home.announcement.announcements")}</CardTitle>
          <CardDescription className="text-xs">{t("home.announcement.latestNews")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-1 flex-col items-center justify-between gap-2 pt-0 px-3">
          <div className="flex flex-1 items-center justify-center w-full rounded-lg border p-2 overflow-hidden">
            <AnimatePresence mode="wait">
              <motion.div
                key={current}
                variants={slideLeftContent}
                initial="initial"
                animate="animate"
                exit="exit"
              >
                <h3 className="font-medium text-xs">{displayedTitle}</h3>
                <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
                  {displayedContent}
                </p>
              </motion.div>
            </AnimatePresence>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="icon-sm" onClick={prev}>
              <ChevronLeft className="size-3" />
            </Button>
            <span className="text-xs text-muted-foreground">
              {current + 1} / {total}
            </span>
            <Button variant="outline" size="icon-sm" onClick={next}>
              <ChevronRight className="size-3" />
            </Button>
          </div>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="aspect-square shadow-sm flex flex-col">
      <CardHeader className="pb-3">
        <CardTitle className="text-base">{t("home.announcement.announcements")}</CardTitle>
        <CardDescription className="text-xs">{t("home.announcement.latestNewsAndUpdates")}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col items-center justify-between gap-3 pt-0">
        <div className="flex flex-1 items-center justify-center w-full rounded-lg border p-3 overflow-hidden">
          <AnimatePresence mode="wait">
            <motion.div
              key={current}
              variants={slideLeftContent}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <h3 className="font-medium text-sm">{displayedTitle}</h3>
              <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
                {displayedContent}
              </p>
            </motion.div>
          </AnimatePresence>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="icon-sm" onClick={prev}>
            <ChevronLeft className="size-4" />
          </Button>
          <span className="text-xs text-muted-foreground">
            {current + 1} / {total}
          </span>
          <Button variant="outline" size="icon-sm" onClick={next}>
            <ChevronRight className="size-4" />
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
