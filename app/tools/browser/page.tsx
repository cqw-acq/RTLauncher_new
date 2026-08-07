"use client";

import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function BrowserPage() {
  useEffect(() => {
    // 拦截 window.open 和 target=_blank 的导航
    const originalOpen = window.open;
    window.open = function (url, target, features) {
      if (url) {
        invoke("log_navigation", { url: String(url) });
        // 如果是外部链接，直接在当前窗口打开
        window.location.href = String(url);
      }
      return null;
    };

    // 拦截点击事件，处理 target=_blank
    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const link = target.closest('a[target="_blank"], a[target="blank"]');
      if (link && link.getAttribute("href")) {
        e.preventDefault();
        const url = link.getAttribute("href");
        if (url) {
          invoke("log_navigation", { url });
          window.location.href = url;
        }
      }
    };

    document.addEventListener("click", handleClick);

    return () => {
      window.open = originalOpen;
      document.removeEventListener("click", handleClick);
    };
  }, []);

  return (
    <div className="h-full w-full bg-background flex items-center justify-center">
      <div className="text-center">
        <p className="text-muted-foreground">正在加载...</p>
      </div>
    </div>
  );
}