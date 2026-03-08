import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:aria-invalid:border-destructive/50 rounded-4xl border border-transparent bg-clip-padding text-sm font-medium focus-visible:ring-[3px] aria-invalid:ring-[3px] [&_svg:not([class*='size-'])]:size-3.5 sm:[&_svg:not([class*='size-'])]:size-4 inline-flex items-center justify-center whitespace-nowrap transition-all disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none shrink-0 [&_svg]:shrink-0 outline-none group/button select-none touch-manipulation",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/80",
        outline: "border-border bg-input/30 hover:bg-input/50 hover:text-foreground aria-expanded:bg-muted aria-expanded:text-foreground",
        secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80 aria-expanded:bg-secondary aria-expanded:text-secondary-foreground",
        ghost: "hover:bg-muted hover:text-foreground dark:hover:bg-muted/50 aria-expanded:bg-muted aria-expanded:text-foreground",
        destructive: "bg-destructive/10 hover:bg-destructive/20 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/20 text-destructive focus-visible:border-destructive/40 dark:hover:bg-destructive/30",
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: cn(
          "h-9 px-3 gap-1.5",
          "sm:h-9 sm:px-3",
          "md:h-10 md:px-4",
          "lg:h-10 lg:px-4",
          "has-data-[icon=inline-end]:pr-2.5 has-data-[icon=inline-start]:pl-2.5"
        ),
        xs: cn(
          "h-6 px-2 gap-1 text-xs",
          "sm:h-6 sm:px-2.5",
          "md:h-7 md:px-2.5",
          "[&_svg:not([class*='size-'])]:size-3"
        ),
        sm: cn(
          "h-7 px-2.5 gap-1",
          "sm:h-8 sm:px-3",
          "md:h-8 md:px-3",
          "has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2"
        ),
        lg: cn(
          "h-10 px-4 gap-1.5",
          "sm:h-11 sm:px-5",
          "md:h-12 md:px-6",
          "[&_svg:not([class*='size-'])]:size-4 sm:[&_svg:not([class*='size-'])]:size-5",
          "has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3"
        ),
        icon: cn(
          "size-8",
          "sm:size-9",
          "md:size-10"
        ),
        "icon-xs": cn(
          "size-5",
          "sm:size-6",
          "[&_svg:not([class*='size-'])]:size-3"
        ),
        "icon-sm": cn(
          "size-7",
          "sm:size-8"
        ),
        "icon-lg": cn(
          "size-10",
          "sm:size-11",
          "md:size-12"
        ),
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
