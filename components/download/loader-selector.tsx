"use client";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { LOADER_OPTIONS } from "@/constants/data";
import type { LoaderType } from "@/types";
import {
  Box,
  Hammer,
  Ribbon,
  Feather,
  Flame,
  Zap,
  Sun,
  Download,
} from "lucide-react";

const loaderIcons: Record<LoaderType, React.ReactNode> = {
  vanilla: <Box className="size-5" />,
  forge: <Hammer className="size-5" />,
  fabric: <Ribbon className="size-5" />,
  quilt: <Feather className="size-5" />,
  neoforge: <Flame className="size-5" />,
  liteloader: <Zap className="size-5" />,
  optifine: <Sun className="size-5" />,
};

const loaderColors: Record<LoaderType, { bg: string; text: string; border: string }> = {
  vanilla: { bg: "bg-stone-100 dark:bg-stone-800/40", text: "text-stone-600 dark:text-stone-400", border: "hover:border-stone-400 dark:hover:border-stone-500" },
  forge: { bg: "bg-orange-100 dark:bg-orange-900/30", text: "text-orange-600 dark:text-orange-400", border: "hover:border-orange-400 dark:hover:border-orange-500" },
  fabric: { bg: "bg-amber-100 dark:bg-amber-900/30", text: "text-amber-600 dark:text-amber-400", border: "hover:border-amber-400 dark:hover:border-amber-500" },
  quilt: { bg: "bg-purple-100 dark:bg-purple-900/30", text: "text-purple-600 dark:text-purple-400", border: "hover:border-purple-400 dark:hover:border-purple-500" },
  neoforge: { bg: "bg-red-100 dark:bg-red-900/30", text: "text-red-600 dark:text-red-400", border: "hover:border-red-400 dark:hover:border-red-500" },
  liteloader: { bg: "bg-cyan-100 dark:bg-cyan-900/30", text: "text-cyan-600 dark:text-cyan-400", border: "hover:border-cyan-400 dark:hover:border-cyan-500" },
  optifine: { bg: "bg-yellow-100 dark:bg-yellow-900/30", text: "text-yellow-600 dark:text-yellow-400", border: "hover:border-yellow-400 dark:hover:border-yellow-500" },
};

interface LoaderSelectorProps {
  versionId: string;
}

export function LoaderSelector({ versionId }: LoaderSelectorProps) {
  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3">
      {LOADER_OPTIONS.map((loader) => {
        const colors = loaderColors[loader.id];
        return (
          <Card
            key={loader.id}
            className={cn(
              "group cursor-pointer transition-all duration-200 hover:shadow-md",
              colors.border
            )}
          >
            <CardHeader className="p-4 pb-2">
              <div className="flex items-center justify-between">
                <div
                  className={cn(
                    "flex size-10 items-center justify-center rounded-lg transition-transform duration-200 group-hover:scale-110",
                    colors.bg
                  )}
                >
                  <span className={colors.text}>{loaderIcons[loader.id]}</span>
                </div>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="opacity-0 group-hover:opacity-100 transition-opacity duration-200"
                  aria-label={`使用 ${loader.name} 安装 ${versionId}`}
                >
                  <Download className="size-4" />
                </Button>
              </div>
              <CardTitle className="text-sm mt-2">{loader.name}</CardTitle>
            </CardHeader>
            <CardContent className="p-4 pt-0">
              <CardDescription className="text-xs line-clamp-2">
                {loader.description}
              </CardDescription>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
