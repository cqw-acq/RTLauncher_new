"use client";

import { cn } from "@/lib/utils";

export type LoaderKind =
  | "vanilla"
  | "forge"
  | "fabric"
  | "quilt"
  | "neoforge"
  | "liteloader"
  | "optifine";

interface LoaderIconProps {
  kind: LoaderKind | string;
  className?: string;
}

const ICON_SRC: Record<LoaderKind, string> = {
  vanilla: "/vanilla.png",
  forge: "/forge.png",
  fabric: "/fabric.png",
  quilt: "/quilt.png",
  neoforge: "/neoforge.png",
  liteloader: "/liteloader.png",
  optifine: "/optifine.png",
};

const LOADER_NAMES: Record<LoaderKind, string> = {
  vanilla: "Vanilla",
  forge: "Forge",
  fabric: "Fabric",
  quilt: "Quilt",
  neoforge: "NeoForge",
  liteloader: "LiteLoader",
  optifine: "OptiFine",
};

function ImageLoaderIcon({
  kind,
  className,
}: {
  kind: LoaderKind;
  className?: string;
}) {
  return (
    <img
      src={ICON_SRC[kind]}
      alt={LOADER_NAMES[kind]}
      className={cn("size-full object-contain", className)}
      draggable={false}
    />
  );
}

const ICONS: Record<LoaderKind, React.FC<{ className?: string }>> = {
  vanilla: (p) => <ImageLoaderIcon kind="vanilla" className={p.className} />,
  forge: (p) => <ImageLoaderIcon kind="forge" className={p.className} />,
  fabric: (p) => <ImageLoaderIcon kind="fabric" className={p.className} />,
  quilt: (p) => <ImageLoaderIcon kind="quilt" className={p.className} />,
  neoforge: (p) => <ImageLoaderIcon kind="neoforge" className={p.className} />,
  liteloader: (p) => <ImageLoaderIcon kind="liteloader" className={p.className} />,
  optifine: (p) => <ImageLoaderIcon kind="optifine" className={p.className} />,
};

export function LoaderIcon({ kind, className }: LoaderIconProps) {
  const k = (kind || "vanilla").toLowerCase() as LoaderKind;
  const Comp = ICONS[k] ?? ICONS.vanilla;
  return (
    <div
      className={cn(
        "flex shrink-0 items-center justify-center overflow-hidden rounded-[6px] select-none",
        className
      )}
      style={{ backgroundColor: "transparent" }}
    >
      <Comp className="size-full" />
    </div>
  );
}

/** 从 loadName（如 "1.20.1-forge-xxx"）或 loader 字符串推断类型 */
export function inferLoaderKind(src: string | null | undefined): LoaderKind {
  if (!src) return "vanilla";
  const s = src.toLowerCase();
  if (s.includes("neoforge")) return "neoforge";
  if (s.includes("liteloader")) return "liteloader";
  if (s.includes("optifine")) return "optifine";
  if (s.includes("fabric")) return "fabric";
  if (s.includes("quilt")) return "quilt";
  if (s.includes("forge")) return "forge";
  if (s.includes("vanilla") || s === "0" || s === "原版") return "vanilla";
  return "vanilla";
}