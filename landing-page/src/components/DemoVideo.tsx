import { DEMO_VIDEO } from "@/config";

/**
 * The 61-second demo. Same-origin file, no autoplay (it has sound), nothing
 * loads until the visitor presses play beyond the poster frame.
 */
export function DemoVideo() {
  return (
    <section className="page watch" id="demo" aria-labelledby="watch-title">
      <h2 className="h2" id="watch-title">
        See a day with it in {DEMO_VIDEO.seconds} seconds.
      </h2>
      <p className="watch__lede">
        One note from a code editor, three from a call, a polish on the last one, then Today, the
        inbox, search and the six looks. It has sound.
      </p>
      <div className="watch__frame">
        <video
          className="watch__video"
          controls
          playsInline
          preload="none"
          poster={DEMO_VIDEO.poster}
          aria-label="Posthaste demo: capturing notes while coding and in a call, then Today, the inbox, search and the looks"
        >
          <source src={DEMO_VIDEO.src} type="video/mp4" />
          <a href={DEMO_VIDEO.src}>Download the demo video (MP4)</a>
        </video>
      </div>
      <p className="quiet watch__note">
        Built from the app&apos;s own windows running with sample notes, shown on a Mac. The code
        editor and the call are mock-ups, and the sound is made up for the film; the app itself is
        silent.
      </p>
    </section>
  );
}
