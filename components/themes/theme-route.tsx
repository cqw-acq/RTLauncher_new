"use client";

import {
  useSyncExternalStore,
  type ComponentType,
  type ReactNode,
} from "react";
import type {
  CoreRouteId,
  ThemePageRegistration,
  ThemeRouteComponentProps,
  ThemeRouteRegistration,
} from "@/lib/themes/protocol";
import type { ThemeRouteRegistry } from "@/lib/themes/route-registry";
import { ThemeErrorBoundary } from "./theme-error-boundary";

interface ThemeRouteProps {
  registry: ThemeRouteRegistry;
  owner: string;
  routeId?: CoreRouteId;
  pathname?: string;
  params?: Readonly<Record<string, string>>;
  search?: URLSearchParams;
  children?: ReactNode;
  onError?: (error: Error, contributionId: string) => void;
}

function renderRouteContribution(
  registration: ThemeRouteRegistration | ThemePageRegistration,
  props: ThemeRouteComponentProps,
  fallback: ReactNode,
  onError?: (error: Error, contributionId: string) => void,
): ReactNode {
  const Contribution = registration.component as ComponentType<ThemeRouteComponentProps>;
  return (
    <ThemeErrorBoundary
      key={registration.id}
      contributionId={registration.id}
      fallback={fallback}
      onError={onError}
    >
      <Contribution {...props} />
    </ThemeErrorBoundary>
  );
}

export function ThemeRoute({
  registry,
  owner,
  routeId,
  pathname,
  params = {},
  search = new URLSearchParams(),
  children,
  onError,
}: ThemeRouteProps) {
  useSyncExternalStore(
    (listener) => registry.subscribe(listener),
    () => registry.getSnapshot(),
    () => registry.getSnapshot(),
  );
  const builtIn = children ?? null;

  if (pathname) {
    const page = registry.resolveVirtualRoute(pathname, owner);
    if (page) {
      const content = renderRouteContribution(
        page,
        { routeId: page.id, params, search },
        builtIn,
        onError,
      );
      return (
        <div data-testid="route-result" data-theme-route={page.id} style={{ display: "contents" }}>
          {content}
        </div>
      );
    }
  }

  if (!routeId) {
    return <div data-testid="route-result" style={{ display: "contents" }}>{builtIn}</div>;
  }

  const resolved = registry.resolveCoreRoute(routeId, owner);
  let content = resolved.replacement
    ? renderRouteContribution(
        resolved.replacement,
        { routeId, params, search, children: builtIn },
        builtIn,
        onError,
      )
    : builtIn;
  content = [...resolved.wrappers].reverse().reduce<ReactNode>(
    (current, wrapper) =>
      renderRouteContribution(
        wrapper,
        { routeId, params, search, children: current },
        current,
        onError,
      ),
    content,
  );

  return (
    <div data-testid="route-result" data-theme-route={routeId} style={{ display: "contents" }}>
      {content}
    </div>
  );
}
