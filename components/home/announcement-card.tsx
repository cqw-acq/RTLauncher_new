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
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useState } from "react";
import { slideLeftContent } from "@/lib/motion";

export function AnnouncementCard() {
  const [current, setCurrent] = useState(0);

  const prev = () =>
    setCurrent((i) => (i - 1 + ANNOUNCEMENTS.length) % ANNOUNCEMENTS.length);
  const next = () =>
    setCurrent((i) => (i + 1) % ANNOUNCEMENTS.length);

  return (
    <Card className="aspect-square shadow-sm flex flex-col">
      <CardHeader className="pb-3">
        <CardTitle className="text-base">公告栏</CardTitle>
        <CardDescription className="text-xs">最新消息和更新</CardDescription>
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
              <h3 className="font-medium text-sm">{ANNOUNCEMENTS[current].title}</h3>
              <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
                {ANNOUNCEMENTS[current].content}
              </p>
            </motion.div>
          </AnimatePresence>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="icon-sm" onClick={prev}>
            <ChevronLeft className="size-4" />
          </Button>
          <span className="text-xs text-muted-foreground">
            {current + 1} / {ANNOUNCEMENTS.length}
          </span>
          <Button variant="outline" size="icon-sm" onClick={next}>
            <ChevronRight className="size-4" />
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}