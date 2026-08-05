"use client";

import { cn } from "@/lib/utils";

interface ProgressProps {
  value?: number;
  className?: string;
}

export function Progress({ value, className }: ProgressProps) {
  const isIndeterminate = value === undefined || Number.isNaN(value);
  const clampedValue = isIndeterminate ? 0 : Math.min(100, Math.max(0, value));

  return (
    <div
      className={cn(
        "relative h-2 w-full overflow-hidden rounded-full bg-secondary",
        className
      )}
    >
      <div
        className={cn(
          "h-full rounded-full bg-primary transition-all duration-300 ease-out",
          isIndeterminate && "animate-pulse opacity-70"
        )}
        style={{
          width: isIndeterminate ? "40%" : `${clampedValue}%`,
          ...(isIndeterminate ? {
            animation: "indeterminate-progress 1.5s ease-in-out infinite",
          } : {}),
        }}
      />
      <style>{`
        @keyframes indeterminate-progress {
          0% { transform: translateX(-100%); }
          100% { transform: translateX(250%); }
        }
      `}</style>
    </div>
  );
}