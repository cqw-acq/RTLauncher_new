import * as React from "react";
import { cn } from "@/lib/utils";

type AlertVariant = "default" | "destructive";

function Alert({
  className,
  variant = "default",
  ...props
}: React.ComponentProps<"div"> & { variant?: AlertVariant }) {
  return (
    <div
      data-slot="alert"
      data-variant={variant}
      role="alert"
      className={cn(
        "relative flex w-full flex-col gap-1 rounded-2xl border bg-card px-4 py-3 text-sm [&>svg]:text-foreground",
        variant === "destructive"
          ? "border-destructive/50 bg-destructive/10 text-destructive [&>svg]:text-destructive"
          : "border-border",
        className
      )}
      {...props}
    />
  );
}

function AlertTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-title"
      className={cn("font-medium leading-none", className)}
      {...props}
    />
  );
}

function AlertDescription({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="alert-description"
      className={cn("text-sm opacity-90", className)}
      {...props}
    />
  );
}

export { Alert, AlertTitle, AlertDescription };