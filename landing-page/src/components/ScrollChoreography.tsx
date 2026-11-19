"use client";

// Subtle, high-craft scroll choreography for the landing page:
// 1. A 2px reading progress bar pinned to the top edge (driven by scrollY).
// 2. A Posthaste ambient glow + floating keycap glyphs that drift and illuminate
//    gently in the background as you scroll, adapting automatically to the
//    active Look's `--accent` token.
// 3. Smooth IntersectionObserver reveals for sections and staggered cards.
// 4. Respects prefers-reduced-motion and never blocks initial visibility if JS
//    has not hydrated or is disabled.

import { useEffect, useRef } from "react";

const REVEAL_SELECTOR = [
  "main > section:not(.hero)",
  ".moment",
  ".feature",
  ".look-tile",
  ".compare__scroll",
  ".compare__picks > div",
  ".release",
  ".free-card",
].join(", ");

const FLOATING_KEYS = [
  { label: "⌘⇧Space", cls: "ambient-key ambient-key--1" },
  { label: "Enter ↵", cls: "ambient-key ambient-key--2" },
  { label: "X", cls: "ambient-key ambient-key--3" },
  { label: "L", cls: "ambient-key ambient-key--4" },
  { label: "Esc", cls: "ambient-key ambient-key--5" },
];

export function ScrollChoreography() {
  const barRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    let raf = 0;
    const updateProgress = () => {
      raf = 0;
      const doc = document.documentElement;
      const max = doc.scrollHeight - window.innerHeight;
      const ratio = max > 0 ? Math.min(1, Math.max(0, window.scrollY / max)) : 0;
      if (barRef.current) {
        barRef.current.style.transform = `scaleX(${ratio})`;
      }
      if (!reduce) {
        const y = Math.min(window.scrollY, 700);
        doc.style.setProperty("--scroll-y", `${y}px`);
        doc.style.setProperty("--scroll-p", ratio.toFixed(4));
        // Gentle horizontal/vertical sweep for the ambient glow orb as the user scrolls.
        const glowX = 18 + Math.sin(ratio * Math.PI * 2.2) * 34 + ratio * 32;
        const glowY = 14 + ratio * 68;
        doc.style.setProperty("--glow-x", `${glowX.toFixed(1)}%`);
        doc.style.setProperty("--glow-y", `${glowY.toFixed(1)}%`);
      }
    };

    const onScroll = () => {
      if (!raf) raf = window.requestAnimationFrame(updateProgress);
    };

    updateProgress();
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll, { passive: true });

    if (reduce || typeof IntersectionObserver === "undefined") {
      return () => {
        if (raf) window.cancelAnimationFrame(raf);
        window.removeEventListener("scroll", onScroll);
        window.removeEventListener("resize", onScroll);
      };
    }

    const targets = Array.from(document.querySelectorAll<HTMLElement>(REVEAL_SELECTOR));

    for (const el of targets) {
      el.setAttribute("data-reveal", "");
      const parent = el.parentElement;
      if (
        parent &&
        (el.matches(".moment") ||
          el.matches(".feature") ||
          el.matches(".look-tile") ||
          el.matches(".release"))
      ) {
        const siblings = Array.from(parent.children).filter((c) => c.matches(el.tagName));
        const idx = siblings.indexOf(el);
        if (idx > 0) {
          el.style.setProperty("--reveal-delay", `${Math.min(idx * 75, 300)}ms`);
        }
      }
    }

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            entry.target.setAttribute("data-revealed", "true");
            observer.unobserve(entry.target);
          }
        }
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.06 },
    );

    const viewH = window.innerHeight;
    for (const el of targets) {
      const rect = el.getBoundingClientRect();
      if (rect.top < viewH * 0.94) {
        el.setAttribute("data-revealed", "true");
      } else {
        observer.observe(el);
      }
    }

    return () => {
      if (raf) window.cancelAnimationFrame(raf);
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
      observer.disconnect();
    };
  }, []);

  return (
    <>
      <div ref={barRef} className="scroll-progress" aria-hidden="true" />
      <div className="ambient-stage" aria-hidden="true">
        <div className="ambient-glow ambient-glow--primary" />
        <div className="ambient-glow ambient-glow--secondary" />
        <div className="ambient-streak" />
        {FLOATING_KEYS.map((k) => (
          <span key={k.label} className={k.cls}>
            {k.label}
          </span>
        ))}
      </div>
    </>
  );
}
