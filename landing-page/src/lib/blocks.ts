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
    fact: "One shortcut over any app. Type, hit Enter, back to work.",
  },
  {
    id: "inbox",
    title: "Group & find fast",
    fact: "Organize by project or day. Type any word to find it.",
  },
  {
    id: "details",
    title: "Steps, details & files",
    fact: "Break notes into steps, add notes, and attach files — saved as you type.",
  },
  {
    id: "undo",
    title: "30-day Trash",
    fact: "Deleted notes wait 30 days in Trash so you can restore anytime.",
  },
  {
    id: "ai",
    title: "Optional AI polish",
    fact: "Turn rough notes into clear sentences — only when you click.",
  },
  {
    id: "looks",
    title: "Make it yours",
    fact: "Six built-in themes or craft your own colors. Press 1–6 to try.",
  },
  {
    id: "keys",
    title: "Built for your keyboard",
    fact: "Every action has a shortcut. Press ? anytime to see them all.",
  },
];

export const BLOCKS_BY_LANG: Record<Lang, Block[]> = {
  en: BLOCKS,
  vi: [
    {
      id: "capture",
      title: "Ghi nhanh trong tích tắc",
      fact: "Một phím tắt trên mọi ứng dụng. Gõ, nhấn Enter, quay lại việc.",
    },
    {
      id: "inbox",
      title: "Nhóm & tìm cực nhanh",
      fact: "Nhóm theo dự án hoặc ngày. Gõ vài chữ là thấy ngay.",
    },
    {
      id: "details",
      title: "Các bước, chi tiết & tệp",
      fact: "Chia nhỏ các bước, viết mô tả và đính kèm tệp — tự lưu khi gõ.",
    },
    {
      id: "undo",
      title: "Thùng rác 30 ngày",
      fact: "Ghi chú đã xóa giữ 30 ngày trong Thùng rác để khôi phục bất cứ lúc nào.",
    },
    {
      id: "ai",
      title: "AI chỉnh câu chữ",
      fact: "Biến ghi chú vội thành câu rõ ràng — tùy chọn, chỉ chạy khi bấm.",
    },
    {
      id: "looks",
      title: "Đẹp theo gu của bạn",
      fact: "6 giao diện có sẵn hoặc tự phối màu. Nhấn 1–6 để thử ngay.",
    },
    {
      id: "keys",
      title: "Làm mọi thứ bằng phím",
      fact: "Mọi thao tác đều có phím tắt. Nhấn ? để xem toàn bộ.",
    },
  ],
};
