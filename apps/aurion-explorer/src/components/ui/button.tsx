import * as React from "react"
import { Slot } from "@radix-ui/react-slot"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded text-xs font-mono font-medium transition-colors disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-3.5 shrink-0 [&_svg]:shrink-0 outline-none cursor-pointer border",
  {
    variants: {
      variant: {
        default:
          "bg-[#1a7f37] dark:bg-[#238636] text-white border-[#1a7f37] dark:border-[#2ea043] hover:bg-[#15672c] dark:hover:bg-[#2ea043]",
        destructive:
          "bg-[#cf222e] dark:bg-[#da3633] text-white border-[#cf222e] dark:border-[#f85149] hover:bg-[#a40e26] dark:hover:bg-[#f85149]",
        outline:
          "border-[#d0d7de] dark:border-[#30363d] bg-white dark:bg-transparent text-[#656d76] dark:text-[#8b949e] hover:text-[#1f2328] dark:hover:text-[#f0f6fc] hover:bg-[#f6f8fa] dark:hover:bg-[#161b22]",
        secondary:
          "border-[#d0d7de] dark:border-[#30363d] bg-[#f6f8fa] dark:bg-[#21262d] text-[#24292f] dark:text-[#c9d1d9] hover:bg-[#eaeef2] dark:hover:bg-[#30363d] hover:text-[#1f2328] dark:hover:text-[#f0f6fc]",
        ghost:
          "border-transparent hover:bg-[#f6f8fa] dark:hover:bg-[#21262d] text-[#656d76] dark:text-[#8b949e] hover:text-[#1f2328] dark:hover:text-[#f0f6fc]",
        link:
          "border-transparent text-[#0969da] dark:text-[#58a6ff] underline-offset-4 hover:underline",
        live:
          "bg-[#dafbe1] dark:bg-[#238636]/20 text-[#1a7f37] dark:text-[#3fb950] border-[#4ac26b]/40 dark:border-[#238636]/60 shadow-xs",
        paused:
          "bg-[#eaeef2] dark:bg-[#21262d] text-[#656d76] dark:text-[#8b949e] border-[#d0d7de] dark:border-[#30363d]",
      },
      size: {
        default: "h-7 px-2.5 py-1",
        sm: "h-6 px-2 py-0.5 text-[11px]",
        lg: "h-8 px-3 py-1.5",
        icon: "h-7 w-7 p-0",
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
  variant,
  size,
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot : "button"

  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
