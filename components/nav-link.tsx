"use client";

import { forwardRef } from "react";

type NavLinkProps = React.AnchorHTMLAttributes<HTMLAnchorElement> & {
  href: string;
};

/** 内部链接统一补尾斜杠*/
function normalizeHref(href: string): string {
  if (!href.startsWith("/")) return href;        // 外链 / 锚点原样返回
  if (href === "/") return href;
  if (href.includes("#") || href.includes("?")) return href;
  return href.endsWith("/") ? href : `${href}/`;
}

/**
 * 内部导航链接：用原生 <a> 作为整页导航，避开打包版下 Next 客户端路由
 * 出现数据取到了但不提交的问题（见 #99）。
 */
export const NavLink = forwardRef<HTMLAnchorElement, NavLinkProps>(
  function NavLink({ href, ...rest }, ref) {
    return <a ref={ref} href={normalizeHref(href)} {...rest} />;
  },
);