// The story page's words and its switch. Copy is data so the page, the
// links to it and the tests all read one place. Same honesty rules as the
// rest of the site: no invented proof, nothing the app does not do.

export interface StoryBeat {
  id: string;
  heading: string;
  /** One string per paragraph. */
  body: string[];
}

export interface Story {
  /** Off until the maker has read the page and wants it live. */
  published: boolean;
  title: string;
  description: string;
  lede: string;
  /** One sentence in the maker's own voice for the line under the hero buttons. Optional. */
  teaser: string;
  /** Two or three sentences in the maker's voice for the note above the free card. Optional. */
  note: string;
  beats: StoryBeat[];
}

export const STORY: Story = {
  published: false,
  title: "Why Posthaste",
  description: "What Posthaste is for, what it leaves out, and how it is made.",
  lede: "What it is for, what it leaves out, and how it is made.",
  teaser: "",
  note: "",
  beats: [
    {
      id: "for",
      heading: "What it is for",
      body: [
        "Posthaste is for the thing you want to write down before you lose it. Press a shortcut over any app, type, press Enter, and you are back where you were.",
        "Later, open it to see what is left for today, tick things off, and search the rest.",
      ],
    },
    {
      id: "left-out",
      heading: "What it leaves out",
      body: [
        "No account, no analytics, no telemetry. Your notes, drafts and looks live in one file on your computer, and export and backup are in Settings.",
        "AI polish is something you switch on. It is off by default, it uses your own key, and it sends only the one note you polish to the provider you chose.",
      ],
    },
    {
      id: "made",
      heading: "How it is made",
      body: [
        "Posthaste is a Tauri 2 app. The core is Rust over SQLite, and the interface is Leptos compiled to WebAssembly. It is MIT licensed and the code is on GitHub.",
        "A look is plain data: a small file you can share as one line of text, or as a file that carries its images.",
      ],
    },
  ],
};

/** The page and every link to it. `STORY_PREVIEW=1` shows them while unpublished. */
export function storyVisible(): boolean {
  return STORY.published || process.env.STORY_PREVIEW === "1";
}

/** The maker's one line under the hero buttons. Null while the story is hidden or has no teaser. */
export function storyHeroLine(): string | null {
  return storyVisible() && STORY.teaser ? STORY.teaser : null;
}
