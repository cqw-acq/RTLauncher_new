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

const FADE_OUT_MS = 140
const FADE_IN_MS = 240
const SETTLE_MS = 400
const STABILITY_MS = 150
const TIMEOUT_MS = 3000
// 内容持续变动超过该时长且达到该次数时视为"流动内容"（如日志实时刷新），
// 不再要求静止，避免导航长时间停在旧页面。
const FLOWING_AFTER_MS = 1000
const FLOWING_MUTATIONS = 4

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
  previousText: string,
  targetPathname: string,
  signal: AbortSignal
) {
  return new Promise<void>((resolve) => {
    let settled = false
    let contentChanged = false
    let contentChangedAt = 0
    let changeCount = 0
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

    // 新页面往往先渲染出加载中指示器(spinner)再显示真实内容；
    // 内容尚未变化前保持等待，避免淡入的是空白/加载圈。
    // 内容一旦变化就按"保持稳定"收尾，忽略残留的小型 spinner
    // （如按钮内的加载图标），否则会被它们无限期阻塞。
    const trySettle = (now: number) => {
      if (!contentChanged) return
      if (!contentChangedAt) contentChangedAt = now
      // 内容持续流动（日志实时刷新等）超过阈值时不再等待静止。
      if (
        now - contentChangedAt > FLOWING_AFTER_MS &&
        changeCount >= FLOWING_MUTATIONS
      ) {
        finish()
        return
      }
      // 收尾时刻 = 首次内容变化的固定窗口(覆盖新页面入场动画，避免淡入
      // 截在动画中途导致淡入结束后内容跳动) 与 最近一次变动后的短暂静止窗口
      // (吸收异步数据波) 的较晚者。入场动画期间的高频样式变动不会无限叠加等待。
      const deadline = Math.max(
        contentChangedAt + SETTLE_MS,
        now + STABILITY_MS
      )
      window.clearTimeout(settleTimer)
      settleTimer = window.setTimeout(finish, Math.max(0, deadline - performance.now()))
    }

    // 用 rAF 批量合并同一帧内的多次 DOM 变动，
    // 避免实时日志等高频更新场景下逐条重算 innerText 卡顿主线程。
    let pending = false
    const observer = new MutationObserver(() => {
      if (pending) return
      pending = true
      requestAnimationFrame(() => {
        pending = false
        if (settled) return
        // 路由尚未切换到目标页面时忽略所有变动：
        // 路由切换期间旧页面自身的异步更新（下载进度、日志等）也会触发
        // MutationObserver，不区分会导致"新页面已就绪"的误判，淡入的仍是旧页面。
        if (normalizePathname(window.location.pathname) !== targetPathname) return
        if (main.innerText === previousText) return
        changeCount += 1
        contentChanged = true
        trySettle(performance.now())
      })
    })

    observer.observe(main, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
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

    const previousText = main.innerText

    abortActiveNavigation()

    const controller = new AbortController()

    // 尊重系统减弱动效设置：直接切换，不做任何过渡动画。
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      router.push(item.href)
      return
    }

    // 统一采用"旧页淡出 → 路由切换 → 新页内容就绪后淡入"的确定性时序，
    // 不再使用 View Transitions 交叉淡入：
    // 旧页彻底不可见之后新页才出现，两者永不同屏，因此不会出现
    // 旧快照叠在新页面之上造成的"旧页闪现"，也不存在滚动位置错位抖动。
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

    window.setTimeout(async () => {
      // 若在此期间已被新的导航中断（abort），不再执行本次路由切换。
      if (controller.signal.aborted) return

      const contentReady = waitForPageContent(
        main,
        previousText,
        targetPathname,
        controller.signal
      )
      router.push(item.href)
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
    }, FADE_OUT_MS + 20)
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
