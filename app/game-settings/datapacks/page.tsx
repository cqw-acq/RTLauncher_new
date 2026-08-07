"use client";

import React from "react";
import { Database } from "lucide-react";
import { createResourcePage } from "@/components/resource-page-factory";

export default createResourcePage({
  title: "panel.datapacks",
  leftIcon: <Database className="size-5 text-amber-500" />,
  leftIconBg: "bg-amber-500/10",
  leftIconColor: "text-amber-500",
  instanceSubdir: "datapacks",
  cacheKind: "datapack",
  extensions: ["zip", "jar"],
  simplifyName: (name) => name.replace(/\.(zip|jar)$/i, ""),
});
