import * as React from "react"
import { cn } from "@/lib/utils"

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        "bg-white dark:bg-[#0d1117] border border-[#d0d7de] dark:border-[#30363d] rounded px-2.5 py-1 text-xs font-mono text-[#24292f] dark:text-[#c9d1d9] placeholder:text-[#656d76] dark:placeholder:text-[#8b949e] focus:outline-none focus:border-[#0969da] dark:focus:border-[#58a6ff] transition-colors disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
}

export { Input }
