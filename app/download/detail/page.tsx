"use client";

import ModDetailContent from "./ModDetailContent";
import { Suspense, useEffect, useState } from "react";

function ModDetailInner() {
  const [modId, setModId] = useState<string>("");

  useEffect(() => {
    if (typeof window !== "undefined") {
      const params = new URLSearchParams(window.location.search);
      const mod = params.get("mod") || "";
      if (mod) setModId(mod);
    }
  }, []);

  if (!modId) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground">
        <p>Please select a mod first</p>
      </div>
    );
  }
  return <ModDetailContent modId={modId} />;
}

export default function ModDetailPage() {
  return (
    <Suspense fallback={<div className="flex flex-1 items-center justify-center text-muted-foreground"><p>Loading...</p></div>}>
      <ModDetailInner />
    </Suspense>
  );
}