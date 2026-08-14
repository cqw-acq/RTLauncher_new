"use client";

import {
  useSyncExternalStore,
  type ComponentType,
  type ReactNode,
} from "react";
import type {
  CoreSlotId,
  ThemeSlotComponentProps,
  ThemeSlotRegistration,
} from "@/lib/themes/protocol";
import type { ThemeSlotRegistry } from "@/lib/themes/slot-registry";
import { ThemeErrorBoundary } from "./theme-error-boundary";

interface ThemeSlotProps<T> {
  registry: ThemeSlotRegistry;
  owner: string;
  slotId: CoreSlotId;
  data?: T;
  children?: ReactNode;
  onError?: (error: Error, contributionId: string) => void;
}

function renderContribution<T>(
  registration: ThemeSlotRegistration,
  slotId: CoreSlotId,
  data: T,
  children: ReactNode,
  fallback: ReactNode,
  onError?: (error: Error, contributionId: string) => void,
): ReactNode {
  const Contribution = registration.component as ComponentType<
    ThemeSlotComponentProps<T>
  >;
  return (
    <ThemeErrorBoundary
      key={registration.id}
      contributionId={registration.id}
      fallback={fallback}
      onError={onError}
    >
      <Contribution slotId={slotId} data={data}>
        {children}
      </Contribution>
    </ThemeErrorBoundary>
  );
}

export function ThemeSlot<T = unknown>({
  registry,
  owner,
  slotId,
  data,
  children,
  onError,
}: ThemeSlotProps<T>) {
  useSyncExternalStore(
    (listener) => registry.subscribe(listener),
    () => registry.getSnapshot(),
    () => registry.getSnapshot(),
  );
  const resolved = registry.resolve(slotId, owner);
  const builtIn = children ?? null;
  let content = resolved.replacement
    ? renderContribution(
        resolved.replacement,
        slotId,
        data as T,
        builtIn,
        builtIn,
        onError,
      )
    : builtIn;

  content = [...resolved.wrappers].reverse().reduce<ReactNode>(
    (current, wrapper) =>
      renderContribution(wrapper, slotId, data as T, current, current, onError),
    content,
  );

  return (
    <div data-testid="slot-result" data-theme-slot={slotId} style={{ display: "contents" }}>
      {resolved.before.map((registration) =>
        renderContribution(registration, slotId, data as T, null, null, onError),
      )}
      {content}
      {resolved.after.map((registration) =>
        renderContribution(registration, slotId, data as T, null, null, onError),
      )}
    </div>
  );
}
