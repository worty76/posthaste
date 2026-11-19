"use client";

// Subtle, high-craft scroll choreography for the landing page:
// 1. A 2px reading progress bar pinned to the top edge (driven by scrollY).
// 2. Smooth, theme-aware IntersectionObserver reveals for sections and staggered
//    children (moments, feature cards, look tiles, comparison, changelog).
// 3. Respects prefers-reduced-motion and never blocks initial visibility if JS
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
      // Parallax drift on the hero keyboard backdrop while hero is in view.
      if (!reduce) {
        const y = Math.min(window.scrollY, 700);
        doc.style.setProperty("--scroll-y", `${y}px`);
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

    // Group sibling cards so they stagger cleanly as a unit when scrolling in.
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

  return <div ref={barRef} className="scroll-progress" aria-hidden="true" />;
}
