import { DEMO_VIDEO } from "@/config";

/**
 * The demo film. Same-origin file, no autoplay (it has sound), nothing
 * loads until the visitor presses play beyond the poster frame.
 */
export function DemoVideo() {
  return (
    <section className="page watch" id="demo" aria-labelledby="watch-title">
      <h2 className="h2" id="watch-title">
        See a day with it in {DEMO_VIDEO.seconds} seconds.
      </h2>
      <p className="watch__lede">
        A note caught from a code editor, Today, a note's details with attachments, why your notes
        never leave your computer, the looks, and a first glimpse of the phone app, which is still
        in development. It has sound.
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
      <p className="quiet watch__note">
        Built from the app&apos;s own windows running with sample notes, shown on a Mac. The code
        editor and the call are mock-ups, and the sound is made up for the film; the app itself is
        silent.
      </p>
    </section>
  );
}
