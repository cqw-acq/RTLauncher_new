"use client";

import * as React from "react";
import { cn } from "@/lib/utils";

const Switch = React.forwardRef<
  HTMLButtonElement,
  React.ButtonHTMLAttributes<HTMLButtonElement> & {
    checked?: boolean;
    onCheckedChange?: (checked: boolean) => void;
  }
>(({ className, checked, onCheckedChange, ...props }, ref) => {
  return (
    <button
      role="switch"
      aria-checked={checked}
      onClick={() => onCheckedChange?.(!checked)}
      className={cn(
        "peer inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary",
        checked ? "bg-primary" : "bg-muted",
        className
      )}
      {...props}
      ref={ref}
    >
      <span
        className={cn(
          "pointer-events-none block h-4 w-4 transform rounded-full bg-white shadow-sm ring-0 transition-transform data-[state=checked]:translate-x-5",
          checked ? "translate-x-5" : "translate-x-0.5"
        )}
      />
    </button>
  );
});

Switch.displayName = "Switch";

export { Switch };