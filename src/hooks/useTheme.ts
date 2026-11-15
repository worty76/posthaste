import { useEffect } from "react";
import type { Theme } from "../lib/types";

export function resolveTheme(theme: Theme, systemDark: boolean): "light" | "dark" {
  return theme === "system" ? (systemDark ? "dark" : "light") : theme;
}

/** Applies the theme to <html>, following the OS while set to "system".
    `forced` pins light or dark regardless of the setting and the OS: a
    look's base wins while a look is active. With `enabled` false nothing is
    written, leaving whatever the page already has. */
export function useTheme(
  theme: Theme,
  forced: "light" | "dark" | null = null,
  enabled: boolean = true,
) {
  useEffect(() => {
    if (!enabled) return;
    if (forced) {
      document.documentElement.dataset.theme = forced;
      return;
    }
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme = resolveTheme(theme, media.matches);
    };
    apply();
    if (theme !== "system") return;
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme, forced, enabled]);
}
