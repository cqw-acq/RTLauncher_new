"use client";

import { motion } from "framer-motion";
import { VersionListItem } from "@/components/download/version-list-item";
import { PackageOpen } from "lucide-react";
import { fadeSlideUp } from "@/lib/motion";
import type { MinecraftVersion } from "@/types";

interface VersionListProps {
  versions: MinecraftVersion[];
  onSelectVersion: (version: MinecraftVersion) => void;
}

export function VersionList({ versions, onSelectVersion }: VersionListProps) {
  if (versions.length === 0) {
    return (
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground py-16"
      >
        <PackageOpen className="size-10 opacity-40" />
        <p className="text-sm">没有找到匹配的版本</p>
      </motion.div>
    );
  }

  return (
    <motion.div
      className="flex-1 min-h-0 overflow-y-auto rounded-xl border border-border bg-card"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1, transition: { duration: 0.15 } }}
    >
      {versions.map((version) => (
        <VersionListItem
          key={version.id}
          version={version}
          onSelect={onSelectVersion}
        />
      ))}
    </motion.div>
  );
}
