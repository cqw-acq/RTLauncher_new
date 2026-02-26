import { useState, useCallback, useEffect } from "react";
import type { ViewType } from "@/types";
import { STORAGE_KEYS } from "@/constants";

/**
 * 管理主页和实例设置视图切换的 Hook
 */
export function useViewToggle() {
  const [currentView, setCurrentView] = useState<ViewType>("home");

  // 在客户端恢复保存的视图状态
  useEffect(() => {
    if (typeof window !== "undefined") {
      try {
        const savedView = localStorage.getItem(
          STORAGE_KEYS.CURRENT_VIEW
        ) as ViewType;
        if (savedView === "home" || savedView === "instance") {
          setCurrentView(savedView);
        }
      } catch (error) {
        console.warn(
          "Failed to read view state from localStorage:",
          error
        );
      }
    }
  }, []);

  const toggleView = useCallback(() => {
    const newView: ViewType = currentView === "home" ? "instance" : "home";
    setCurrentView(newView);
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEYS.CURRENT_VIEW, newView);
    }
  }, [currentView]);

  const setView = useCallback((view: ViewType) => {
    setCurrentView(view);
    if (typeof window !== "undefined") {
      localStorage.setItem(STORAGE_KEYS.CURRENT_VIEW, view);
    }
  }, []);

  return {
    currentView,
    toggleView,
    setView,
  };
}
