"use client";

import { Download, Loader2, Star, Trash2, X } from "lucide-react";
import { useState } from "react";
import { useDownloadManager } from "@/components/download/download-provider";
import { Button } from "@/components/ui/button";
import { useModFavorites } from "@/hooks/use-mod-favorites";
import { useI18n } from "@/components/i18n/use-i18n";

export function FavoriteModList() {
  const { favorites, removeFavorite } = useModFavorites();
  const { startResourceDownload } = useDownloadManager();
  const { t } = useI18n();
  const [downloading, setDownloading] = useState(false);
  const [showTip, setShowTip] = useState(true);

  const downloadAll = async () => {
    if (!favorites.length || downloading) return;
    setDownloading(true);
    try {
      await Promise.all(
        favorites.map((favorite) =>
          startResourceDownload(
            "mod",
            favorite.slug,
            favorite.name,
            favorite.mcVersion,
            favorite.modLoader,
            favorite.downloadUrl,
          ),
        ),
      );
    } finally {
      setDownloading(false);
    }
  };

  return (
    <div className="mt-4 flex min-h-0 flex-1 flex-col gap-4">
      {showTip && (
        <div className="flex items-center gap-2 rounded-lg border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-xs text-amber-800 dark:text-amber-300">
          <Star className="size-3.5 shrink-0 fill-amber-400 text-amber-500" aria-hidden="true" />
          <span className="min-w-0 flex-1">{t("download.favorites.starTip")}</span>
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-6 shrink-0 text-amber-800 hover:bg-amber-500/15 hover:text-amber-900 dark:text-amber-300 dark:hover:text-amber-100"
            title={t("download.favorites.closeTip")}
            aria-label={t("download.favorites.closeTip")}
            onClick={() => setShowTip(false)}
          >
            <X className="size-3.5" />
          </Button>
        </div>
      )}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold">{t("download.favorites.title")}</h2>
          <p className="mt-1 text-xs text-muted-foreground">
            {t("download.favorites.savedCount", { count: favorites.length })}
          </p>
        </div>
        <Button
          size="sm"
          className="gap-1.5"
          disabled={!favorites.length || downloading}
          onClick={() => void downloadAll()}
        >
          {downloading ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
          {t("download.favorites.downloadAllToCache")}
        </Button>
      </div>

      {favorites.length ? (
        <div className="overflow-hidden rounded-xl border border-border bg-card">
          <div className="divide-y divide-border">
            {favorites.map((favorite) => (
              <div key={favorite.id} className="flex items-center gap-3 px-4 py-3">
                <Star className="size-4 shrink-0 fill-amber-400 text-amber-500" aria-hidden="true" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{favorite.name}</p>
                  <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
                    {favorite.versionLabel} · MC {favorite.mcVersion} · {favorite.modLoader}
                  </p>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className="shrink-0 gap-1.5"
                  onClick={() => void startResourceDownload("mod", favorite.slug, favorite.name, favorite.mcVersion, favorite.modLoader, favorite.downloadUrl)}
                >
                  <Download className="size-3.5" />
                  {t("download.favorites.download")}
                </Button>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="size-7 shrink-0 text-muted-foreground hover:text-destructive"
                  title={t("download.favorites.removeFromFavorites")}
                  aria-label={t("download.favorites.removeFromFavoritesNamed", { name: favorite.name })}
                  onClick={() => removeFavorite(favorite.id)}
                >
                  <Trash2 className="size-3.5" />
                </Button>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-border bg-card/50 p-8 text-center">
          <Star className="size-7 text-muted-foreground" />
          <p className="text-sm font-medium">{t("download.favorites.empty")}</p>
          <p className="text-xs text-muted-foreground">{t("download.favorites.emptyHint")}</p>
        </div>
      )}
    </div>
  );
}
