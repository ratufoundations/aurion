import * as React from "react"
import { Slot } from "@radix-ui/react-slot"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center justify-center rounded px-2 py-0.5 text-xs font-mono font-medium w-fit whitespace-nowrap shrink-0 [&>svg]:size-3 gap-1 [&>svg]:pointer-events-none transition-colors border",
  {
    variants: {
      variant: {
        default:
          "border-transparent bg-[#0969da] dark:bg-[#1f6feb] text-white",
        secondary:
          "border-[#d0d7de] dark:border-[#30363d] bg-[#f6f8fa] dark:bg-[#161b22] text-[#656d76] dark:text-[#8b949e]",
        destructive:
          "border-[#ff8182]/40 dark:border-[#f85149]/50 bg-[#ffebe9] dark:bg-[#f85149]/20 text-[#cf222e] dark:text-[#f85149]",
        warning:
          "border-[#d4a72c]/40 dark:border-[#d29922]/50 bg-[#fff8c5] dark:bg-[#d29922]/20 text-[#9a6700] dark:text-[#d29922]",
        success:
          "border-[#4ac26b]/40 dark:border-[#238636]/50 bg-[#dafbe1] dark:bg-[#238636]/20 text-[#1a7f37] dark:text-[#3fb950]",
        info:
          "border-[#54aeff]/40 dark:border-[#388bfd]/50 bg-[#ddf4ff] dark:bg-[#388bfd]/15 text-[#0969da] dark:text-[#58a6ff]",
        outline:
          "border-[#d0d7de] dark:border-[#30363d] text-[#24292f] dark:text-[#c9d1d9]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

function Badge({
  className,
  variant,
  asChild = false,
  ...props
}: React.ComponentProps<"span"> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "span"

  return (
    <Comp
      data-slot="badge"
      className={cn(badgeVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Badge, badgeVariants }
