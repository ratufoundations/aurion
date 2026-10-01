"use client";

import { useEffect, useState } from "react";
import App from "@/App";

export default function Page() {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) {
    return (
      <div
        className="min-h-screen bg-[#f6f8fa] dark:bg-[#0d1117] text-[#0969da] dark:text-[#58a6ff] flex items-center justify-center font-mono text-sm"
        translate="no"
        suppressHydrationWarning
      >
        <div className="flex items-center gap-2" suppressHydrationWarning>
          <span className="inline-block w-2.5 h-2.5 rounded-full bg-[#1a7f37] dark:bg-[#3fb950] animate-ping" />
          <span suppressHydrationWarning>AURION EXPLORER MESH</span>
        </div>
      </div>
    );
  }

  return <App />;
}
