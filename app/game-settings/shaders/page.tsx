"use client";

import React from "react";
import { Layers } from "lucide-react";
import { createResourcePage } from "@/components/resource-page-factory";

export default createResourcePage({
  title: "panel.shaders",
  leftIcon: <Layers className="size-5 text-purple-500" />,
  leftIconBg: "bg-purple-500/10",
  leftIconColor: "text-purple-500",
  instanceSubdir: "shaderpacks",
  cacheKind: "shaderpack",
  extensions: ["zip", "jar"],
  simplifyName: (name) => name.replace(/\.(zip|jar)$/i, ""),
});
