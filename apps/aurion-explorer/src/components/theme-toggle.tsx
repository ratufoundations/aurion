"use client";

import React, { useEffect, useState } from "react";
import { useTheme } from "next-themes";
import { Sun, Moon } from "lucide-react";
import { Button } from "@/components/ui/button";

export function ThemeToggle() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return (
      <Button
        variant="outline"
        size="sm"
        className="h-7 px-2 text-xs font-mono border-[#d0d7de] dark:border-[#30363d] text-[#656d76] dark:text-[#8b949e]"
        aria-label="Toggle Theme"
      >
        <span className="w-3.5 h-3.5 inline-block" />
        <span className="hidden sm:inline">Theme</span>
      </Button>
    );
  }

  const isDark = (theme === "system" ? resolvedTheme : theme) === "dark";

  return (
    <Button
      variant="outline"
      size="sm"
      onClick={() => setTheme(isDark ? "light" : "dark")}
      className="h-7 px-2.5 text-xs font-mono flex items-center gap-1.5 border-[#d0d7de] dark:border-[#30363d] bg-white dark:bg-[#161b22] text-[#24292f] dark:text-[#f0f6fc] hover:bg-[#f6f8fa] dark:hover:bg-[#21262d] transition-all cursor-pointer shadow-xs"
      title={isDark ? "Beralih ke Mode Terang (Light Mode)" : "Beralih ke Mode Gelap (Dark Mode)"}
    >
      {isDark ? (
        <>
          <Sun className="w-3.5 h-3.5 text-[#e3b341] transition-transform rotate-0 hover:rotate-45" />
          <span className="text-[11px] font-semibold text-[#8b949e] hover:text-[#f0f6fc]">GELAP</span>
        </>
      ) : (
        <>
          <Moon className="w-3.5 h-3.5 text-[#0969da] transition-transform -rotate-12 hover:rotate-0" />
          <span className="text-[11px] font-semibold text-[#656d76] hover:text-[#1f2328]">TERANG</span>
        </>
      )}
    </Button>
  );
}
