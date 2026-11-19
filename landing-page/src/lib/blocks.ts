// Copy for the "what it does" blocks — short, plain, human-readable copy
// that non-technical visitors can scan quickly, in English and Vietnamese.

import type { Lang } from "./i18n";

export type BlockId = "capture" | "inbox" | "details" | "undo" | "looks" | "ai" | "keys";

export interface Block {
  id: BlockId;
  title: string;
  fact: string;
}

export const BLOCKS: Block[] = [
  {
    id: "capture",
    title: "Capture in a second",
    fact: "One shortcut over any app. Type, press Enter, and you're right back to work.",
  },
  {
    id: "inbox",
    title: "Find anything fast",
    fact: "Notes stay grouped by day in your inbox. Type any word to find it right away.",
  },
  {
    id: "details",
    title: "Add details & files",
    fact: "Write longer notes and attach photos or PDFs — everything saves as you type.",
  },
  {
    id: "undo",
    title: "Delete without worry",
    fact: "Deleted notes wait 30 days in Trash so you can restore them anytime.",
  },
  {
    id: "ai",
    title: "Clean up rough notes",
    fact: "Optional AI polish turns quick typing into clear sentences — only when you click.",
  },
  {
    id: "looks",
    title: "Make it yours",
    fact: "Switch between 6 built-in themes or craft your own colors. Press 1–6 to try now.",
  },
  {
    id: "keys",
    title: "Built for your keyboard",
    fact: "Do everything without touching the mouse. Press ? anytime to see every shortcut.",
  },
];

export const BLOCKS_BY_LANG: Record<Lang, Block[]> = {
  en: BLOCKS,
  vi: [
    {
      id: "capture",
      title: "Ghi nhanh trong tích tắc",
      fact: "Một phím tắt trên mọi ứng dụng. Gõ, nhấn Enter, rồi quay lại việc ngay.",
    },
    {
      id: "inbox",
      title: "Tìm lại cực nhanh",
      fact: "Ghi chú xếp gọn theo ngày. Chỉ cần gõ vài chữ là thấy ngay.",
    },
    {
      id: "details",
      title: "Thêm chi tiết & tệp",
      fact: "Viết mô tả dài và đính kèm ảnh hoặc PDF — tự động lưu khi bạn gõ.",
    },
    {
      id: "undo",
      title: "Xóa không lo mất",
      fact: "Ghi chú đã xóa nằm 30 ngày trong Thùng rác để bạn khôi phục bất cứ lúc nào.",
    },
    {
      id: "ai",
      title: "Chỉnh câu chữ gọn đẹp",
      fact: "AI tùy chọn giúp biến ghi chú vội thành câu rõ ràng — chỉ chạy khi bạn bấm.",
    },
    {
      id: "looks",
      title: "Đẹp theo gu của bạn",
      fact: "Đổi qua lại 6 giao diện có sẵn hoặc tự pha màu. Nhấn phím 1–6 để thử ngay.",
    },
    {
      id: "keys",
      title: "Làm mọi thứ bằng bàn phím",
      fact: "Thao tác mượt không cần chuột. Nhấn ? bất cứ lúc nào để xem phím tắt.",
    },
  ],
};
