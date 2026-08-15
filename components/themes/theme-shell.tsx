"use client";

import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import { PageTransition } from "@/components/page-transition";
import { Sidebar } from "@/components/sidebar";
import { TitleBar } from "@/components/title-bar";
import type { CoreRouteId } from "@/lib/themes/protocol";
import { ThemeRoute } from "./theme-route";
import { useThemeRuntime } from "./theme-runtime-provider";
import { ThemeSlot } from "./theme-slot";

function routeIdFromPathname(pathname: string): CoreRouteId | undefined {
  if (pathname === "/") return "core.home";
  if (pathname === "/launch") return "core.launch";
  if (pathname === "/download/detail") return "core.download.detail";
  if (pathname === "/download") return "core.download";
  if (pathname === "/multiplayer") return "core.multiplayer";
  if (pathname === "/tools") return "core.tools";
  if (pathname === "/settings") return "core.settings";
  if (pathname === "/game-settings") return "core.game-settings";
  const instanceSection = pathname.match(
    /^\/(?:game-settings|instance-settings)\/(mods|worlds|resources|shaders|screenshots|schematics)/,
  )?.[1];
  if (instanceSection) return `core.instance.${instanceSection}` as CoreRouteId;
  return undefined;
}

export function ThemeShell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const { routes, slots, snapshot, reportThemeError } = useThemeRuntime();
  const owner = snapshot.activeOwner;

  return (
    <>
      <TitleBar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex-1 overflow-hidden [view-transition-name:page-content]">
          <ThemeSlot registry={slots} owner={owner} slotId="page.header" onError={reportThemeError} />
          <ThemeSlot registry={slots} owner={owner} slotId="page.header.actions" onError={reportThemeError} />
          <ThemeSlot registry={slots} owner={owner} slotId="app.content.before" onError={reportThemeError} />
          <ThemeRoute
            registry={routes}
            owner={owner}
            routeId={routeIdFromPathname(pathname)}
            pathname={pathname}
            onError={reportThemeError}
          >
            <PageTransition>{children}</PageTransition>
          </ThemeRoute>
          <ThemeSlot registry={slots} owner={owner} slotId="app.content.after" onError={reportThemeError} />
          <ThemeSlot registry={slots} owner={owner} slotId="page.footer" onError={reportThemeError} />
        </main>
      </div>
    </>
  );
}
