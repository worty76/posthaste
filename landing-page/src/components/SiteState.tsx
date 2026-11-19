"use client";

// Shared state for the page: which look it wears, and the demo notes the
// hero and the Today window both show. Memory only: nothing is stored.

import { createContext, useContext, useEffect, useMemo, useReducer, useState } from "react";
import { DEFAULT_LOOK_ID, LOOKS } from "@/lib/looks";
import { applyLook } from "@/lib/theme";
import { demoReducer, initialDemo, type DemoAction, type DemoState } from "@/lib/demo";
import { COPY, type Lang, type SiteCopy } from "@/lib/i18n";

interface SiteState {
  /** The look the page wears. */
  lookId: string;
  setLook: (id: string) => void;
  /** Current interface language ("en" or "vi"). */
  lang: Lang;
  setLang: (lang: Lang) => void;
  copy: SiteCopy;
  demo: DemoState;
  dispatch: (action: DemoAction) => void;
}

const Ctx = createContext<SiteState | null>(null);

export function useSite(): SiteState {
  const value = useContext(Ctx);
  if (!value) throw new Error("useSite outside SiteStateProvider");
  return value;
}

/** The interface copy alone, for pages that render parts of the shell
    without the provider (the story page's footer): English there. */
export function useCopy(): SiteCopy {
  return useContext(Ctx)?.copy ?? COPY.en;
}

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

export function SiteStateProvider({ children }: { children: React.ReactNode }) {
  const [demo, dispatch] = useReducer(demoReducer, undefined, initialDemo);
  // The page starts in Ink through CSS alone; data-look is only set once
  // the visitor picks another look.
  const [lookId, setLookId] = useState<string>(DEFAULT_LOOK_ID);
  const [chosen, setChosen] = useState(false);
  const [lang, setLang] = useState<Lang>("en");

  useEffect(() => {
    if (chosen) applyLook(lookId);
  }, [chosen, lookId]);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  // 1 to 6 switch looks, like picking from the app's gallery, whenever the
  // visitor is not typing somewhere.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || isTyping(e.target)) return;
      const look = LOOKS[Number(e.key) - 1];
      if (look && /^[1-6]$/.test(e.key)) {
        setChosen(true);
        setLookId(look.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const value = useMemo<SiteState>(
    () => ({
      lookId,
      setLook: (id) => {
        setChosen(true);
        setLookId(id);
      },
      lang,
      setLang,
      copy: COPY[lang],
      demo,
      dispatch,
    }),
    [lookId, lang, demo],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}
