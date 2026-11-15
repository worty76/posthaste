import { useCallback, useEffect, useState } from "react";
import { applyLook } from "../lib/appearance";
import { api, on } from "../lib/ipc";
import type { ResolvedLook, Settings } from "../lib/types";
import { useTheme } from "./useTheme";

/**
 * Applies the active look to the window and keeps it current. A preview from
 * the editor overrides the active look until cleared. While a look is active
 * its base pins light or dark; with Plain (no look), the theme setting
 * follows the OS as before. Nothing is touched until the active look is
 * known: index.html paints the first frame as Ink, the factory default, and
 * clearing that early would flash the OS theme and font.
 */
export function useAppearance(settings: Settings | null): {
  setPreview: (look: ResolvedLook | null) => void;
} {
  const [active, setActive] = useState<ResolvedLook | null>(null);
  const [preview, setPreview] = useState<ResolvedLook | null>(null);
  const [known, setKnown] = useState(false);

  const lookId = settings?.look_id ?? null;

  const reload = useCallback(() => {
    // A rejected lookup (corrupt row, unknown id) is just "no look".
    api.lookActive().then(
      (look) => {
        setActive(look);
        setKnown(true);
      },
      () => {
        setActive(null);
        setKnown(true);
      },
    );
  }, []);

  useEffect(() => {
    void reload();
    return on("looks-changed", reload);
  }, [lookId, reload]);

  const effective = preview ?? active;

  useEffect(() => {
    if (!known) return;
    applyLook(effective);
    return () => applyLook(null);
  }, [effective, known]);

  useTheme(settings?.theme ?? "system", effective?.base ?? null, known);

  const setPreviewLook = useCallback((look: ResolvedLook | null) => setPreview(look), []);
  return { setPreview: setPreviewLook };
}
