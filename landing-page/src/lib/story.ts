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
  description:
    "I type fast and live on my computer, and I could not find a to-do app that felt right. So I built one.",
  lede: "I built Posthaste for myself. This is why, and how it turned out.",
  teaser: "I built Posthaste for myself, to manage tasks without leaving the keyboard.",
  note: "I'm always on my computer and I type fast, so I didn't want to reach for the mouse just to open a to-do app. I couldn't find one that felt right, so I built Posthaste for myself. I hope you enjoy it too.",
  beats: [
    {
      id: "why",
      heading: "Why I built it",
      body: [
        "I'm always on my computer, and I type pretty fast. I don't want to reach for my mouse just to open a to-do app and manage my tasks while I'm working on something else.",
        "But finding a note-taking app that felt right for me was surprisingly difficult. I wanted something free, minimal, fast, and good-looking. Most apps I tried felt too complicated or came with things I simply didn't need.",
        "So I decided to build one, inspired by Zed and Vim, two of my favorite tools.",
      ],
    },
    {
      id: "name",
      heading: "The name and the feeling",
      body: [
        "I named it Posthaste, which means doing something immediately or with great speed.",
        "As a developer, I'm a big fan of keyboard-first workflows, especially the experience of using the Zed editor. I wanted that same feeling in a notes app: manage my to-do notes quickly without interrupting my coding flow.",
      ],
    },
    {
      id: "different",
      heading: "A different approach",
      body: [
        "I know there are already plenty of apps out there, but I wanted to take a different approach. No unnecessary complexity, no forced AI, just a fast and thoughtful place to capture, organize, and manage your notes on a daily basis.",
        "Free, open-source, local-first, keyboard-first, and built with love for the little details.",
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
      ],
    },
    {
      id: "join",
      heading: "Help make it better",
      body: [
        "I built Posthaste for myself. Now, I hope you'll enjoy using it too.",
        "If you find it useful, I'd love your ideas, bug reports, or help making it better for future users. And if you like the project, a star on GitHub would mean a lot and give me extra motivation to keep working on it.",
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
