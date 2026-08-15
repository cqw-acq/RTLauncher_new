"use client";

import { useEffect } from "react";

export function useMultiplayerLogPolling(
  active: boolean,
  pollLog: () => Promise<string>
) {
  useEffect(() => {
    if (!active) return;

    void pollLog();
    const timer = window.setInterval(() => {
      void pollLog();
    }, 1_000);

    return () => window.clearInterval(timer);
  }, [active, pollLog]);
}
