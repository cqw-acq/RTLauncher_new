"use client";

import { forwardRef } from "react";
import { useRouter } from "next/navigation";

type NavLinkProps = React.AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string;
};

/** 内部链接统一添补尾斜杠 */
function normalizeHref(href: string): string {
  if (href.startsWith("//")) return href;
  if (!href.startsWith("/")) return href;
  if (href === "/") return href;
  if (href.includes("#") || href.includes("?")) return href;
  return href.endsWith("/") ? href : `${href}/`;
}

/**
 * 内部导航链接。
 *
 * 用原生 <a> + onClick 里手动 router.push：在保留客户端路由（不整页刷新）的同时，
 * 又绕开 next/link 自带导航在打包版下不提交的问题（见 #99 / PR #107）。
 */
export const NavLink = forwardRef<HTMLAnchorElement, NavLinkProps>(
  function NavLink({ href, onClick, target, ...rest }, ref) {
    const router = useRouter();
    const normalized = normalizeHref(href);
    const isInternal = normalized.startsWith("/") && !normalized.startsWith("//");

    return (
      <a
        ref={ref}
        href={normalized}
        target={target}
        onClick={(event) => {
          onClick?.(event);
          if (!isInternal || event.defaultPrevented) return;
          if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
          if (event.button !== 0) return;
          if (target && target !== "_self") return;
          event.preventDefault();
          router.push(normalized);
        }}
        {...rest}
      />
    );
  },
);