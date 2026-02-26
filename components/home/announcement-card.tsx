"use client";

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

export function AnnouncementCard() {
  const [current, setCurrent] = useState(0);

  const prev = () =>
    setCurrent((i) => (i - 1 + ANNOUNCEMENTS.length) % ANNOUNCEMENTS.length);
  const next = () =>
    setCurrent((i) => (i + 1) % ANNOUNCEMENTS.length);

  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>公告栏</CardTitle>
        <CardDescription>最新消息和更新</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col items-center justify-center gap-4">
        <div className="w-full rounded-xl border p-4">
          <h3 className="font-semibold">{ANNOUNCEMENTS[current].title}</h3>
          <p className="mt-1 text-sm text-muted-foreground">
            {ANNOUNCEMENTS[current].content}
          </p>
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
