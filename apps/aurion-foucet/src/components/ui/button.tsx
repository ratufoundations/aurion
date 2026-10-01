import * as React from "react";
import { cn } from "../../lib/utils";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "destructive" | "outline" | "secondary" | "ghost" | "link" | "glow" | "subtle";
  size?: "default" | "sm" | "lg" | "icon";
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = "default", size = "default", ...props }, ref) => {
    const baseStyles =
      "inline-flex items-center justify-center whitespace-nowrap rounded-xl text-sm font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400/50 disabled:pointer-events-none disabled:opacity-50 select-none cursor-pointer";

    const variants = {
      default:
        "bg-cyan-500 text-zinc-950 font-semibold hover:bg-cyan-400 active:scale-[0.98] shadow-md shadow-cyan-500/20",
      glow:
        "bg-gradient-to-r from-cyan-500 via-emerald-400 to-cyan-500 bg-[length:200%_auto] text-zinc-950 font-bold hover:bg-[position:right_center] shadow-lg shadow-cyan-500/25 hover:shadow-cyan-400/40 active:scale-[0.98] transition-all",
      destructive:
        "bg-red-500/20 text-red-400 border border-red-500/30 hover:bg-red-500/30 active:scale-[0.98]",
      outline:
        "border border-zinc-800 bg-zinc-900/60 text-zinc-300 hover:bg-zinc-800 hover:text-white active:scale-[0.98]",
      secondary:
        "bg-zinc-800/80 text-zinc-200 border border-zinc-700/50 hover:bg-zinc-700 hover:text-white active:scale-[0.98]",
      ghost:
        "text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/60 active:scale-[0.98]",
      subtle:
        "bg-zinc-900/80 text-cyan-400 border border-cyan-500/20 hover:border-cyan-500/40 hover:bg-cyan-950/30 active:scale-[0.98]",
      link: "text-cyan-400 underline-offset-4 hover:underline",
    };

    const sizes = {
      default: "h-11 px-5 py-2.5",
      sm: "h-9 rounded-lg px-3 text-xs",
      lg: "h-13 rounded-xl px-7 text-base font-semibold",
      icon: "h-10 w-10 p-0",
    };

    return (
      <button
        ref={ref}
        className={cn(baseStyles, variants[variant], sizes[size], className)}
        {...props}
      />
    );
  }
);

Button.displayName = "Button";
