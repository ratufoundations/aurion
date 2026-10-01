import * as React from "react";
import { cn } from "../../lib/utils";

export interface BadgeProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: "default" | "secondary" | "destructive" | "outline" | "cyan" | "emerald" | "amber";
}

export function Badge({
  className,
  variant = "default",
  ...props
}: BadgeProps) {
  const variants = {
    default:
      "border-transparent bg-zinc-800 text-zinc-200 hover:bg-zinc-700/80",
    secondary:
      "border-zinc-700/60 bg-zinc-900/80 text-zinc-300",
    destructive:
      "border-red-500/30 bg-red-950/40 text-red-400",
    outline:
      "border-zinc-800 text-zinc-400",
    cyan:
      "border-cyan-500/30 bg-cyan-950/40 text-cyan-300 shadow-sm shadow-cyan-500/10",
    emerald:
      "border-emerald-500/30 bg-emerald-950/40 text-emerald-300 shadow-sm shadow-emerald-500/10",
    amber:
      "border-amber-500/30 bg-amber-950/40 text-amber-300 shadow-sm shadow-amber-500/10",
  };

  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-semibold tracking-wide transition-colors select-none",
        variants[variant],
        className
      )}
      {...props}
    />
  );
}
