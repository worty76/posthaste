"use client";

import { useSite } from "./SiteState";

/** Compact EN / VI switcher in the top header. Memory-only, no cookies. */
export function LangToggle() {
  const { lang, setLang } = useSite();
  return (
    <div className="lang-toggle" role="group" aria-label="Language">
      <button
        type="button"
        className="lang-toggle__btn"
        aria-pressed={lang === "en"}
        onClick={() => setLang("en")}
      >
        EN
      </button>
      <button
        type="button"
        className="lang-toggle__btn"
        aria-pressed={lang === "vi"}
        onClick={() => setLang("vi")}
      >
        VI
      </button>
    </div>
  );
}
