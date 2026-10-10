import { useEffect, useRef, useState } from "react";
import { api } from "../client";
import type { Selection } from "../contracts";

export const selectionKey = (s: Selection) =>
  `${s.sourceId}:${s.frameRef.indexGeneration}:${s.frameRef.presentationOrdinal}`;

/** Reuses bounded, source-revision-aware thumbnails across tray and export views. */
export function useFrameThumbnails(selections: Selection[]) {
  const cache = useRef(new Map<string, string>());
  const [images, setImages] = useState<Record<string, string>>({});
  const signature = selections.map(selectionKey).join("|");
  useEffect(() => {
    let alive = true;
    const items = selections.slice(0, 128);
    void (async () => {
      for (const s of items) {
        if (!alive) return;
        const key = selectionKey(s);
        if (cache.current.has(key)) continue;
        try {
          const image = await api.thumbnail(
            s.sourceId,
            s.frameRef.presentationOrdinal,
          );
          if (!alive) return;
          cache.current.set(key, api.image(image.imagePath));
          if (cache.current.size > 256)
            cache.current.delete(cache.current.keys().next().value!);
          setImages(Object.fromEntries(cache.current));
        } catch {
          // An unavailable source keeps its frame identity; do not invent a preview.
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, [signature]);
  return images;
}
