"use client";

import { useSite } from "./SiteState";
import { DEMO_VIDEO } from "@/config";

/**
 * The demo film. Same-origin file, no autoplay (it has sound), nothing
 * loads until the visitor presses play beyond the poster frame.
 */
export function DemoVideo() {
  const { lang } = useSite();
  const vi = lang === "vi";
  return (
    <section className="page watch" id="demo" aria-labelledby="watch-title">
      <h2 className="h2" id="watch-title">
        {vi
          ? `Xem một ngày dùng Posthaste trong ${DEMO_VIDEO.seconds} giây.`
          : `See a day with it in ${DEMO_VIDEO.seconds} seconds.`}
      </h2>
      <p className="watch__lede">
        {vi
          ? "Ghi nhanh khi đang code, xem việc Hôm nay, mở chi tiết ghi chú và đổi giao diện."
          : "Capture from a code editor, check Today, open note details, and switch looks."}
      </p>
      <div className="watch__frame">
        <video
          className="watch__video"
          controls
          playsInline
          preload="none"
          poster={DEMO_VIDEO.poster}
          aria-label="Posthaste demo: capturing notes while coding and in a call, then Today, a note's details, the inbox, search and the looks"
        >
          <source src={DEMO_VIDEO.src} type="video/mp4" />
          <a href={DEMO_VIDEO.src}>Download the demo video (MP4)</a>
        </video>
      </div>
    </section>
  );
}
