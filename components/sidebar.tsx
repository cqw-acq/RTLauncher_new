"use client"

// 导航图标
import {
  Home,
  Download,
  Rocket,
  Wrench,
  Settings,
  Globe,
} from "lucide-react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import { motion } from "framer-motion"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@/components/ui/avatar"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

interface SidebarProps {
  className?: string
}

interface NavItem {
  icon: React.ReactNode
  label: string
  href: string
  isAvatar?: boolean
}

// 顶部导航项
const topNavItems: NavItem[] = [
  { icon: <Home className="size-4" />, label: "首页", href: "/" },
  { icon: <Rocket className="size-4" />, label: "启动", href: "/launch" },
  { icon: <Download className="size-4" />, label: "下载", href: "/download" },
  { icon: <Globe className="size-4" />, label: "联机", href: "/multiplayer" },
  { icon: <Wrench className="size-4" />, label: "工具", href: "/tools" },
]

// 底部导航项
const bottomNavItems: NavItem[] = [
  { icon: <Settings className="size-4" />, label: "设置", href: "/settings" },
  {
    icon: (
      <Avatar size="sm">
        <AvatarImage src="https://github.com/shadcn.png" alt="User" />
        <AvatarFallback>U</AvatarFallback>
      </Avatar>
    ),
    label: "个人",
    href: "/profile",
    isAvatar: true,
  },
]

// 导航按钮
function NavButton({ item, isActive }: { item: NavItem; isActive: boolean }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Link href={item.href}>
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
            <Button
              variant="ghost"
              size="icon"
              className={cn(
                "relative overflow-hidden",
                isActive && "text-accent-foreground"
              )}
            >
              {isActive && (
                <motion.span
                  layoutId="active-nav-indicator"
                  className="absolute inset-0 rounded-md bg-accent"
                  transition={{ type: "spring", stiffness: 380, damping: 30 }}
                />
              )}
              <span className="relative z-10">{item.icon}</span>
            </Button>
          )}
        </Link>
      </TooltipTrigger>
      <TooltipContent side="right">
        <p>{item.label}</p>
      </TooltipContent>
    </Tooltip>
  )
}

// 左侧边栏
export function Sidebar({ className }: SidebarProps) {
  const pathname = usePathname()

  const isActive = (href: string) => {
    if (href === "/") return pathname === "/"
    return pathname.startsWith(href)
  }

  return (
    <aside
      className={cn(
        "flex h-full w-14 flex-col border-r border-border bg-sidebar",
        className
      )}
    >
      <nav className="flex flex-1 flex-col items-center gap-2 p-2">
        {topNavItems.map((item) => (
          <NavButton key={item.href} item={item} isActive={isActive(item.href)} />
        ))}
      </nav>

      <div className="flex flex-col items-center gap-2 border-t border-border p-2">
        {bottomNavItems.map((item) => (
          <NavButton key={item.href} item={item} isActive={isActive(item.href)} />
        ))}
      </div>
    </aside>
  )
}
