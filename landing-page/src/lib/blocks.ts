// Copy for the "what it does" blocks, as data so a test can check every
// block has a heading and a sentence naming a key, a number or a behaviour.

export type BlockId = "capture" | "inbox" | "details" | "undo" | "looks" | "ai" | "keys";

export interface Block {
  id: BlockId;
  title: string;
  fact: string;
}

export const BLOCKS: Block[] = [
  {
    id: "capture",
    title: "Quick capture",
    fact: "One shortcut over any app. Enter saves, the window closes, and the note is in your inbox.",
  },
  {
    id: "inbox",
    title: "Inbox and search",
    fact: "Notes wait in the inbox, grouped by day. Search finds one by any word in it.",
  },
  {
    id: "details",
    title: "Details for any note",
    fact: "Add a description and attach images or documents to a note. It saves as you type, and the files stay in your one database file.",
  },
  {
    id: "undo",
    title: "Delete without fear",
    fact: "A deleted note waits 24 hours in the trash, so Undo always works.",
  },
  {
    id: "ai",
    title: "AI polish, when you want it",
    fact: "Off until you add your own key for Anthropic, OpenAI or Gemini. Then only the note you polish is sent.",
  },
  {
    id: "looks",
    title: "Six looks, or your own",
    fact: "Colors, corners and letter shapes change together. Press 1 to 6 and this page changes too.",
  },
  {
    id: "keys",
    title: "Every move is a key",
    fact: "Every global shortcut can be changed in Settings, and ? lists them all inside the app.",
  },
];
