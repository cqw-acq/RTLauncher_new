"use client";

import React from "react";
import { Image as ImageIcon } from "lucide-react";
import { createResourcePage } from "@/components/resource-page-factory";

export default createResourcePage({
  title: "panel.resourcePacks",
  leftIcon: <ImageIcon className="size-5 text-sky-500" />,
  leftIconBg: "bg-sky-500/10",
  leftIconColor: "text-sky-500",
  instanceSubdir: "resourcepacks",
  cacheKind: "resourcepack",
  extensions: ["zip", "jar"],
  simplifyName: (name) => name.replace(/\.(zip|jar)$/i, ""),
});
