import * as React from "react"
import { Button } from "./button"
import { useMediaQuery } from "@/hooks/use-media-query"
import type { VariantProps } from "class-variance-authority"
import { buttonVariants } from "./button"

interface ResponsiveButtonProps extends React.ComponentProps<"button">,
  VariantProps<typeof buttonVariants> {
  /**
   * Control responsive size behavior
   * - "auto": Button size automatically adjusts based on viewport
   * - "fixed": Button size remains as specified
   * @default "auto"
   */
  responsiveSize?: "auto" | "fixed"
  asChild?: boolean
}

/**
 * Enhanced Button component with automatic responsive sizing
 *
 * Automatically adjusts button size based on viewport:
 * - Mobile (≤640px): Scales down large buttons for better fit
 * - Tablet (≤1024px): Medium scaling adjustment
 * - Desktop (>1024px): Uses specified size
 *
 * @example
 * ```tsx
 * // Auto-responsive button (default)
 * <ResponsiveButton size="lg">Launch</ResponsiveButton>
 *
 * // Fixed size (no auto-adjustment)
 * <ResponsiveButton size="lg" responsiveSize="fixed">Launch</ResponsiveButton>
 * ```
 */
export function ResponsiveButton({
  size = "default",
  responsiveSize = "auto",
  asChild = false,
  ...props
}: ResponsiveButtonProps) {
  const isMobile = useMediaQuery("(max-width: 640px)")
  const isTablet = useMediaQuery("(max-width: 1024px)")

  // Fixed size mode: use size as-is
  if (responsiveSize === "fixed") {
    return <Button size={size} asChild={asChild} {...props} />
  }

  // Auto-adjust size based on viewport
  let adjustedSize = size ?? "default"

  if (isMobile) {
    // Mobile: scale down large sizes
    if (size === "lg") adjustedSize = "default"
    else if (size === "default") adjustedSize = "sm"
    else if (size === "icon-lg") adjustedSize = "icon"
    else if (size === "icon") adjustedSize = "icon-sm"
  } else if (isTablet) {
    // Tablet: moderate scaling
    if (size === "lg") adjustedSize = "default"
    else if (size === "icon-lg") adjustedSize = "icon"
  }

  return <Button size={adjustedSize} asChild={asChild} {...props} />
}

