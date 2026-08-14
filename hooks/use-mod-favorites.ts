"use client";

import { useCallback, useEffect, useState } from "react";
import {
  type ModFavorite,
  modFavoritesChangeEvent,
  readModFavorites,
  writeModFavorites,
} from "@/lib/mod-favorites";

export function useModFavorites() {
  const [favorites, setFavorites] = useState<ModFavorite[]>([]);

  useEffect(() => {
    const sync = () => setFavorites(readModFavorites());
    sync();
    window.addEventListener(modFavoritesChangeEvent(), sync);
    window.addEventListener("storage", sync);
    return () => {
      window.removeEventListener(modFavoritesChangeEvent(), sync);
      window.removeEventListener("storage", sync);
    };
  }, []);

  const toggleFavorite = useCallback((favorite: ModFavorite) => {
    const current = readModFavorites();
    const exists = current.some((item) => item.id === favorite.id);
    const next = exists
      ? current.filter((item) => item.id !== favorite.id)
      : [{ ...favorite, addedAt: Date.now() }, ...current];
    writeModFavorites(next);
    setFavorites(next);
    return !exists;
  }, []);

  const removeFavorite = useCallback((id: string) => {
    const next = readModFavorites().filter((item) => item.id !== id);
    writeModFavorites(next);
    setFavorites(next);
  }, []);

  return { favorites, toggleFavorite, removeFavorite };
}
