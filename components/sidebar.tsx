"use client"

import {
  Home,
  Download,
  Rocket,
  Wrench,
  Settings,
  Globe,
  Gamepad2,
} from "lucide-react"
import Link from "next/link"
import { usePathname, useRouter } from "next/navigation"
import { motion } from "framer-motion"
import { cn } from "@/lib/utils"
import { buttonVariants } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useUIConfigContext } from "@/components/ui-config/ui-config-provider"
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n"
import { ThemeSlot } from "@/components/themes/theme-slot"
import { useThemeRuntime } from "@/components/themes/theme-runtime-provider"

interface SidebarProps {
  className?: string
}

interface NavItem {
  id: string
  icon: React.ReactNode
  label: string
  href: string
  isAvatar?: boolean
}

let activeNavigation: {
  controller: AbortController
  previousPointerEvents: string
  previousTransition: string
  previousOpacity: string
} | null = null

const FADE_OUT_MS = 80
const FADE_IN_MS = 150
// 新页面出现首个 mutation 后的稳定窗口 —— 只等很短时间就淡入，
// 让用户尽快看到新页面的结构，剩余内容增量渲染时自然可见。
const FIRST_MUTATION_SETTLE_MS = 80
// 页面切换超时兜底：即使一直没有 mutation 也不再阻塞。
const TIMEOUT_MS = 1200

function abortActiveNavigation() {
  const nav = activeNavigation
  if (!nav) return
  nav.controller.abort()
  const main = document.querySelector<HTMLElement>("main")
  if (main?.isConnected) {
    main.style.pointerEvents = nav.previousPointerEvents
    main.style.transition = nav.previousTransition
    main.style.opacity = nav.previousOpacity
  }
  activeNavigation = null
}

// 归一化路径（去掉结尾斜杠），用于判断路由是否已切换到位。
function normalizePathname(path: string) {
  let normalized = path
  if (normalized.length > 1 && normalized.endsWith("/")) {
    normalized = normalized.slice(0, -1)
  }
  return normalized
}

function waitForPageContent(
  main: HTMLElement,
  targetPathname: string,
  signal: AbortSignal
) {
  return new Promise<void>((resolve) => {
    let settled = false
    let settleTimer: number | undefined

    const finish = () => {
      if (settled) return
      settled = true
      observer.disconnect()
      window.clearTimeout(timeoutTimer)
      window.clearTimeout(settleTimer)
      signal.removeEventListener("abort", finish)
      resolve()
    }

    const observer = new MutationObserver(() => {
      if (settled) return
      if (normalizePathname(window.location.pathname) !== targetPathname) return
      // 首次 mutation 就说明新页面开始渲染了，
      // 设一个很短的稳定窗口吸收连续的初次渲染 mutation。
      if (!settleTimer) {
        settleTimer = window.setTimeout(() => {
          settleTimer = undefined
          finish()
        }, FIRST_MUTATION_SETTLE_MS)
      }
    })

    observer.observe(main, {
      childList: true,
      subtree: true,
      characterData: true,
    })

    const timeoutTimer = window.setTimeout(finish, TIMEOUT_MS)

    if (signal.aborted) {
      finish()
      return
    }

    signal.addEventListener("abort", finish, { once: true })
  })
}

const NAV_ITEM_BASE: Omit<NavItem, "label">[] = [
  { id: "home", icon: <Home className="size-4" />, href: "/" },
  { id: "game-settings", icon: <Gamepad2 className="size-4" />, href: "/game-settings" },
  { id: "launch", icon: <Rocket className="size-4" />, href: "/launch" },
  { id: "download", icon: <Download className="size-4" />, href: "/download" },
  { id: "multiplayer", icon: <Globe className="size-4" />, href: "/multiplayer" },
  { id: "tools", icon: <Wrench className="size-4" />, href: "/tools" },
  { id: "settings", icon: <Settings className="size-4" />, href: "/settings" },
]

const NAV_LABELS: Record<string, TranslationKey> = {
  home: "sidebar.home",
  "game-settings": "sidebar.gameSettings",
  launch: "sidebar.launch",
  download: "sidebar.downloads",
  multiplayer: "sidebar.multiplayer",
  tools: "sidebar.tools",
  settings: "sidebar.settings",
}

function NavButton({ item, isActive, isExactActive }: { item: NavItem; isActive: boolean; isExactActive: boolean }) {
  const router = useRouter()

  const handleNavigation = (event: React.MouseEvent<HTMLAnchorElement>) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return
    }

    if (isExactActive) {
      event.preventDefault()
      return
    }

    event.preventDefault()
    const main = document.querySelector<HTMLElement>("main")
    if (!main) return

    abortActiveNavigation()

    const controller = new AbortController()

    // 尊重系统减弱动效设置：直接切换，不做任何过渡动画。
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      router.push(item.href)
      return
    }

    const previousTransition = main.style.transition
    const previousPointerEvents = main.style.pointerEvents
    const previousOpacity = main.style.opacity

    const navigation = {
      controller,
      previousTransition,
      previousPointerEvents,
      previousOpacity,
    }
    activeNavigation = navigation

    main.style.pointerEvents = "none"
    main.style.transition = `opacity ${FADE_OUT_MS}ms ease-out`
    main.style.opacity = "0"

    const targetPathname = normalizePathname(item.href)

    // 路由切换与淡出动画并行执行：旧页开始淡出的同时，
    // 新页的加载/渲染已经在进行，大大缩短空白窗口。
    router.push(item.href)
    const contentReady = waitForPageContent(
      main,
      targetPathname,
      controller.signal
    )

    window.setTimeout(async () => {
      if (controller.signal.aborted) return
      await contentReady

      if (activeNavigation !== navigation) return
      requestAnimationFrame(() => {
        if (!main.isConnected) return
        main.style.transition = `opacity ${FADE_IN_MS}ms ease-out`
        main.style.opacity = "1"
        window.setTimeout(() => {
          if (main.isConnected) {
            main.style.transition = previousTransition
            main.style.pointerEvents = previousPointerEvents
            main.style.opacity = previousOpacity
          }
          if (activeNavigation === navigation) activeNavigation = null
        }, FADE_IN_MS + 20)
      })
    }, FADE_OUT_MS / 2) // 在淡出动画过半后才开始准备淡入，
    // 避免过早淡入导致旧页半透明残留与新页同时可见。
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Link
          href={item.href}
          onClick={handleNavigation}
          suppressHydrationWarning
          aria-label={item.label}
          aria-current={isActive ? "page" : undefined}
          className={cn(
            item.isAvatar
              ? "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
              : buttonVariants({ variant: "ghost", size: "icon" }),
            "relative overflow-hidden touch-manipulation",
            !item.isAvatar && isActive && "text-accent-foreground"
          )}
        >
          {item.isAvatar ? (
            <span
              className={cn(
                "flex size-9 items-center justify-center rounded-4xl transition-colors duration-200",
                isActive && "ring-2 ring-primary ring-offset-2 ring-offset-sidebar"
              )}
            >
              {item.icon}
            </span>
          ) : (
            <>
              {isActive && (
                <motion.span
                  layoutId="active-nav-indicator"
                  className="absolute inset-0 rounded-md bg-accent"
                  transition={{ type: "spring", stiffness: 380, damping: 30 }}
                />
              )}
              <span className="relative z-10">{item.icon}</span>
            </>
          )}
        </Link>
      </TooltipTrigger>
      <TooltipContent side="right">
        <p>{item.label}</p>
      </TooltipContent>
    </Tooltip>
  )
}

function isNavItemActive(pathname: string, href: string) {
  if (href === "/") return pathname === "/"
  return pathname === href || pathname.startsWith(`${href}/`)
}

function isNavItemExactActive(pathname: string, href: string) {
  return pathname === href
}

export function Sidebar({ className }: SidebarProps) {
  const pathname = usePathname()
  const { config, configLoaded } = useUIConfigContext()
  const { t } = useI18n()
  const { slots, snapshot, reportThemeError } = useThemeRuntime()
  const allNavItems = NAV_ITEM_BASE.map((item) => ({
    ...item,
    label: NAV_LABELS[item.id] ? t(NAV_LABELS[item.id]) : item.id,
  }))

  const isActive = (href: string) => isNavItemActive(pathname, href)
  const isExactActive = (href: string) => isNavItemExactActive(pathname, href)

  // 根据配置过滤可见的导航项
  const visibleNavItems = configLoaded
    ? allNavItems.filter(item => {
        const tabConfig = config.sidebarTabs.find(tab => tab.id === item.id);
        return tabConfig ? tabConfig.visible : true;
      })
    : allNavItems;

  // 分离顶部和底部导航项
  const topNavItems = visibleNavItems.filter(item => item.id !== "settings");
  const bottomNavItems = visibleNavItems.filter(item => item.id === "settings");

  const builtInSidebar = (
    <aside
      data-app-sidebar
      className={cn(
        "flex h-full w-14 flex-col border-r border-border bg-sidebar",
        className
      )}
    >
      <ThemeSlot registry={slots} owner={snapshot.activeOwner} slotId="app.sidebar.header" onError={reportThemeError} />
      <ThemeSlot registry={slots} owner={snapshot.activeOwner} slotId="app.sidebar.navigation" onError={reportThemeError}>
        <nav className="flex flex-1 flex-col items-center gap-2 p-2">
          {topNavItems.map((item) => (
            <NavButton key={item.href} item={item} isActive={isActive(item.href)} isExactActive={isExactActive(item.href)} />
          ))}
        </nav>
      </ThemeSlot>

      <ThemeSlot registry={slots} owner={snapshot.activeOwner} slotId="app.sidebar.footer" onError={reportThemeError}>
        <div className="flex flex-col items-center gap-2 border-t border-border p-2">
          {bottomNavItems.map((item) => (
            <NavButton key={item.href} item={item} isActive={isActive(item.href)} isExactActive={isExactActive(item.href)} />
          ))}
        </div>
      </ThemeSlot>
    </aside>
  )

  return (
    <ThemeSlot registry={slots} owner={snapshot.activeOwner} slotId="app.sidebar" onError={reportThemeError}>
      {builtInSidebar}
    </ThemeSlot>
  )
}
