// The comparison table, as data. Every cell is a plain statement about how an
// app is set up, never a price, a number of seconds or a ranking: those go
// stale or can't be measured here, and the sources we could reach disagree on
// them. The test in tests/compare.test.ts enforces that. When you update a
// cell, check it against the product's own pages and bump COMPARE_CHECKED_ON.

/** The day the competitor cells were last checked. ISO date. */
export const COMPARE_CHECKED_ON = "2026-11-19";

export type CompareId = "posthaste" | "apple-notes" | "notion" | "obsidian" | "todoglow";

export interface CompareApp {
  id: CompareId;
  name: string;
  /** The product's own page, so a visitor can check us. Not set for Posthaste. */
  url?: string;
}

/** Posthaste first, then the strongest competitors people compare it against. */
export const COMPARE_APPS: CompareApp[] = [
  { id: "posthaste", name: "Posthaste" },
  { id: "apple-notes", name: "Apple Notes", url: "https://www.apple.com/apple-notes/" },
  { id: "notion", name: "Notion", url: "https://www.notion.com/pricing" },
  { id: "obsidian", name: "Obsidian", url: "https://obsidian.md/pricing" },
  { id: "todoglow", name: "TodoGlow", url: "https://devglow.app/todoglow" },
];

export interface CompareRow {
  label: string;
  cells: Record<CompareId, string>;
}

export const COMPARE_ROWS: CompareRow[] = [
  {
    label: "Account",
    cells: {
      posthaste: "None",
      "apple-notes": "Apple Account for sync",
      notion: "Required",
      obsidian: "None for local use",
      todoglow: "Required for sync",
    },
  },
  {
    label: "Storage",
    cells: {
      posthaste: "One local file on your computer",
      "apple-notes": "iCloud or Mac",
      notion: "Notion's cloud",
      obsidian: "Local Markdown folder",
      todoglow: "Local, optional cloud sync",
    },
  },
  {
    label: "Cost",
    cells: {
      posthaste: "Free, MIT open source",
      "apple-notes": "Free (iCloud storage paid)",
      notion: "Free tier; paid plans for full AI & teams",
      obsidian: "Free personal; paid sync",
      todoglow: "Free trial, then paid",
    },
  },
  {
    label: "AI",
    cells: {
      posthaste: "Optional, your own key",
      "apple-notes": "Apple Intelligence",
      notion: "Paid plans",
      obsidian: "Community plugins",
      todoglow: "Own Gemini key + MCP",
    },
  },
  {
    label: "Sync & mobile",
    cells: {
      posthaste: "Direct device sync (no cloud); phone app in development",
      "apple-notes": "Mac, iPhone, iPad, web",
      notion: "Desktop, phone, web",
      obsidian: "Desktop & phone (paid sync or DIY)",
      todoglow: "Mac & Windows; no phone app",
    },
  },
];

/** Why you would pick Posthaste. Each line is a behaviour, not a boast. */
export const PICK_POSTHASTE: string[] = [
  "Keyboard-first capture and a short Today list on macOS, Windows, or Linux.",
  "Local data on your computer, direct device sync without cloud storage, no account.",
  "Optional AI polish using your own key.",
];

/** Where the others are the better pick. Said plainly, because it is true. */
export const PICK_OTHERS: string[] = [
  "Phone app today or Apple Pencil support (Apple Notes).",
  "Shared team docs, databases, and wikis (Notion).",
  "Long-form linked Markdown vault (Obsidian).",
  "Focus timer or MCP coding agents (TodoGlow).",
];

/** Vietnamese translation of the comparison table and pick lists. */
export const COMPARE_VI: {
  rows: CompareRow[];
  pickPosthaste: string[];
  pickOthers: string[];
} = {
  rows: [
    {
      label: "Tài khoản",
      cells: {
        posthaste: "Không cần",
        "apple-notes": "Cần Apple ID để đồng bộ",
        notion: "Bắt buộc",
        obsidian: "Không cần nếu dùng cục bộ",
        todoglow: "Cần để đồng bộ",
      },
    },
    {
      label: "Lưu trữ",
      cells: {
        posthaste: "Một tệp ngay trên máy bạn",
        "apple-notes": "iCloud hoặc máy Mac",
        notion: "Máy chủ của Notion",
        obsidian: "Thư mục Markdown trên máy",
        todoglow: "Trên máy, đồng bộ đám mây tùy chọn",
      },
    },
    {
      label: "Chi phí",
      cells: {
        posthaste: "Miễn phí, mã nguồn mở MIT",
        "apple-notes": "Miễn phí (iCloud thêm trả phí)",
        notion: "Bản miễn phí; AI đầy đủ trả phí",
        obsidian: "Miễn phí cá nhân; đồng bộ trả phí",
        todoglow: "Dùng thử, sau đó trả phí",
      },
    },
    {
      label: "AI",
      cells: {
        posthaste: "Tùy chọn, dùng khóa riêng",
        "apple-notes": "Apple Intelligence",
        notion: "Gói trả phí",
        obsidian: "Qua plugin cộng đồng",
        todoglow: "Khóa Gemini riêng + MCP",
      },
    },
    {
      label: "Đồng bộ & điện thoại",
      cells: {
        posthaste:
          "Đồng bộ trực tiếp giữa các máy (không qua đám mây); app điện thoại đang phát triển",
        "apple-notes": "Mac, iPhone, iPad, web",
        notion: "Máy tính, điện thoại, web",
        obsidian: "Máy tính & điện thoại (trả phí hoặc tự làm)",
        todoglow: "Mac & Windows; chưa có app điện thoại",
      },
    },
  ],
  pickPosthaste: [
    "Ghi nhanh bằng phím tắt và danh sách Hôm nay gọn trên macOS, Windows, Linux.",
    "Dữ liệu nằm trên máy bạn, đồng bộ thiết bị không cần đám mây, không cần tài khoản.",
    "AI tùy chọn bằng khóa riêng.",
  ],
  pickOthers: [
    "Cần app điện thoại ngay hoặc vẽ Apple Pencil (Apple Notes).",
    "Làm việc nhóm trên tài liệu và bảng dữ liệu (Notion).",
    "Kho ghi chú Markdown liên kết dài hạn (Obsidian).",
    "Đồng hồ tập trung hoặc kết nối coding agent qua MCP (TodoGlow).",
  ],
};
