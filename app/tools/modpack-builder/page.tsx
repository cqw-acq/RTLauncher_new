"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { ModpackBuilder } from "@/components/modpack/ModpackBuilder";
import {
  ModrinthFileEntry,
  CurseforgeFileEntry,
  loadInstance,
} from "@/components/modpack/modpack-api";
import { Loader2 } from "lucide-react";
import { useI18n } from "@/components/i18n/use-i18n";

function ModpackBuilderInner() {
  const router = useRouter();
  const search = useSearchParams();
  const { t } = useI18n();

  const type = search.get("type");
  const name = search.get("name") || undefined;
  const editExisting = search.get("edit") === "1";

  const [existingFiles, setExistingFiles] = useState<
    (ModrinthFileEntry | CurseforgeFileEntry)[] | null
  >(editExisting && name ? null : []);
  const [existingGV, setExistingGV] = useState<string>("");
  const [existingLoader, setExistingLoader] = useState<string>("");
  const [existingLoaderVersion, setExistingLoaderVersion] = useState<string>("");
  const [existingPackVersion, setExistingPackVersion] = useState<string>("1.0.0");
  const [existingAuthor, setExistingAuthor] = useState<string>("");
  const [existingOptifine, setExistingOptifine] = useState<boolean>(false);
  const [existingOptifineVersion, setExistingOptifineVersion] = useState<string>("");
  const [existingCrossLoader, setExistingCrossLoader] = useState<boolean>(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    if (type !== "modrinth" && type !== "curseforge") {
      router.push("/tools");
      return;
    }
    if (editExisting && name) {
      loadInstance(name)
        .then((inst) => {
          if ((inst as any).format !== type) {
            setLoadError(t("tools.modpackBuilder.theModpackTypeDoesNotMatch"));
            setExistingFiles([]);
            return;
          }
          setExistingFiles(inst.files as any);
          // MC 版本来自 dependencies.minecraft；versionId 是整合包自身版本。
          const gv =
            (inst as any).dependencies?.minecraft ||
            (inst as any).game_version ||
            // 兼容旧工程：旧版错误地把 MC 版本保存在 versionId。
            (inst as any).versionId ||
            "";
          setExistingGV(gv);
          const loadedLoader = (inst as any).loader || "";
          setExistingLoader(loadedLoader);
          const dependencyLoaderVersion =
            loadedLoader === "forge"
              ? (inst as any).dependencies?.forge
              : loadedLoader === "neoforge"
                ? (inst as any).dependencies?.neoforge ||
                  (inst as any).dependencies?.["neoforge-loader"]
                : loadedLoader === "fabric"
                  ? (inst as any).dependencies?.["fabric-loader"]
                  : loadedLoader === "quilt"
                    ? (inst as any).dependencies?.["quilt-loader"]
                    : "";
          const loadedLoaderVersion =
            (inst as any).loader_version || dependencyLoaderVersion || "";
          setExistingLoaderVersion(
            loadedLoaderVersion.toLowerCase() === "latest" ? "" : loadedLoaderVersion,
          );
          const modrinthVersionId = (inst as any).versionId || "";
          setExistingPackVersion(
            (inst as any).format === "modrinth"
              ? modrinthVersionId || "1.0.0"
              : (inst as any).version || "1.0.0",
          );
          setExistingAuthor((inst as any).author || "");
          setExistingOptifine((inst as any).optifine || false);
          setExistingOptifineVersion((inst as any).optifine_version || "");
          setExistingCrossLoader((inst as any).cross_loader || false);
        })
        .catch((e) => {
          setLoadError(String(e));
          setExistingFiles([]);
        });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!type || (type !== "modrinth" && type !== "curseforge")) {
    return null;
  }

  if (existingFiles === null) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-muted-foreground gap-2">
        <Loader2 className="size-4 animate-spin" /> {t("tools.modpackBuilder.loadingModpackData")}
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-red-500">
        {t("tools.modpackBuilder.loadFailed")}{loadError}
      </div>
    );
  }

  return (
    <ModpackBuilder
      format={type}
      initialName={name}
      gameVersion={existingGV || undefined}
      initialLoader={existingLoader || undefined}
      initialLoaderVersion={existingLoaderVersion || undefined}
      initialPackVersion={existingPackVersion}
      initialAuthor={existingAuthor || undefined}
      initialOptifine={existingOptifine}
      initialOptifineVersion={existingOptifineVersion || undefined}
      initialCrossLoader={existingCrossLoader}
      existingFiles={existingFiles}
    />
  );
}

export default function ModpackBuilderPage() {
  const { t } = useI18n();
  return (
    <Suspense fallback={
      <div className="h-full flex items-center justify-center text-sm text-muted-foreground gap-2">
        <Loader2 className="size-4 animate-spin" /> {t("tools.modpackBuilder.loading")}
      </div>
    }>
      <ModpackBuilderInner />
    </Suspense>
  );
}
