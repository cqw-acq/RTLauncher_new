"use client";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { LOADER_OPTIONS } from "@/constants/data";
import type { LoaderType } from "@/types";
import {
  Box,
  Hammer,
  Ribbon,
  Feather,
  Flame,
} from "lucide-react";

const loaderIcons: Record<LoaderType, React.ReactNode> = {
  vanilla: <Box className="size-4" />,
  forge: <Hammer className="size-4" />,
  fabric: <Ribbon className="size-4" />,
  quilt: <Feather className="size-4" />,
  neoforge: <Flame className="size-4" />,
};

interface LoaderSelectorProps {
  selectedLoader: LoaderType;
  onSelect: (loader: LoaderType) => void;
}

export function LoaderSelector({
  selectedLoader,
  onSelect,
}: LoaderSelectorProps) {
  return (
    <div className="flex items-center gap-2">
      {LOADER_OPTIONS.map((loader) => {
        const isActive = selectedLoader === loader.id;
        return (
          <Tooltip key={loader.id}>
            <TooltipTrigger asChild>
              <Button
                variant={isActive ? "default" : "outline"}
                size="sm"
                onClick={() => onSelect(loader.id)}
                className={cn(
                  "gap-2 transition-all duration-200",
                  isActive && "shadow-md"
                )}
              >
                {loaderIcons[loader.id]}
                <span>{loader.name}</span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              <p>{loader.description}</p>
            </TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
