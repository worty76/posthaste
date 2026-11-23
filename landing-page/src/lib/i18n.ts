// English and Vietnamese copy for the landing page.
// Short, human-readable sentences that non-technical visitors can scan in seconds.

export type Lang = "en" | "vi";

export interface SiteCopy {
  nav: {
    why: string;
    features: string;
    looks: string;
    compare: string;
    changelog: string;
    download: string;
  };
  hero: {
    pos: string;
    def: string;
    whyLink: string;
    title: string;
    sub1: string;
    sub2: string;
    sub3: string;
    facts: string[];
    downloadFor: (os: string) => string;
    downloadFree: string;
    watchDemo: string;
  };
  demo: {
    /** The two notes the hero types by itself before the visitor does. */
    notes: [string, string];
  };
  moments: {
    heading: string;
    step1: string;
    step1Desc: string;
    step1Empty: string;
    step1Count: (n: number) => string;
    step2: string;
    step3: string;
    step3Hint: string;
    undo: string;
  };
  features: {
    heading: string;
    looksLink: string;
    looksTail: string;
  };
  looks: {
    heading: string;
    lede: string;
    makeOwn: string;
    makeOwnHelp: string;
    share: string;
  };
  local: {
    heading: string;
    staysTitle: string;
    stays: string[];
    leavesTitle: string;
    leaves: string[];
  };
  sync: {
    heading: string;
    lede: string;
    filmLabel: string;
    points: string[];
    phoneNote: string;
    filmNote: string;
  };
  compare: {
    heading: string;
    lede: string;
    checkedPrefix: string;
    pickUs: string;
    pickOthers: string;
    bothNote1: string;
    bothNote2: string;
  };
  free: {
    heading: string;
    body: string;
    source: string;
    license: string;
    discord: string;
  };
  get: {
    heading: string;
    metaPrefix: string;
    allFiles: string;
    note: string;
    buildSummary: string;
  };
  footer: {
    privacy: string;
    why: string;
    source: string;
  };
}

export const COPY: Record<Lang, SiteCopy> = {
  en: {
    nav: {
      why: "Why Posthaste",
      features: "Features",
      looks: "Looks",
      compare: "Compare",
      changelog: "Changelog",
      download: "Download",
    },
    hero: {
      pos: "adverb",
      def: "with great speed; right away.",
      whyLink: "Why it exists",
      title: "Your to-do notes, at the speed of your keyboard.",
      sub1: "One shortcut over any app. Manage your to-do notes, and you're right back to work.",
      sub2: "Your data is fully local, safe, and can sync across your devices without storing your data in the cloud.",
      sub3: "",
      facts: ["Free", "Minimal", "Fast", "Local", "Customizable", "AI optional", "Keyboard-first"],
      downloadFor: (os) => `Download for ${os}`,
      downloadFree: "Download free",
      watchDemo: "Watch the demo",
    },
    demo: {
      notes: ["Buy oat milk on the way home", "Send Minh the demo link"],
    },
    moments: {
      heading: "Catch it. See what's left. Tick it off.",
      step1: "Catch it",
      step1Desc: "Type, hit Enter, back to work.",
      step1Empty: "Type in the box above — saved notes land in Today.",
      step1Count: (n) => `You caught ${n} ${n === 1 ? "note" : "notes"} on this page.`,
      step2: "See what's left",
      step3: "Tick it off",
      step3Hint: "Select a note and press X.",
      undo: "Undo",
    },
    features: {
      heading: "Small, and it does all of this.",
      looksLink: "Try the six looks below",
      looksTail: ", or press 1–6 anywhere.",
    },
    looks: {
      heading: "Your colors, on all of it.",
      lede: "Click a look (or press 1–6) to retheme this page just like the app.",
      makeOwn: "Or make your own",
      makeOwnHelp: "Pick 4 colors. Low-contrast combinations are caught before saving.",
      share: "Share it as one line",
    },
    local: {
      heading: "On your computer, not ours.",
      staysTitle: "Stays on your computer",
      stays: [
        "Everything lives in one file on your computer — easy to back up anywhere.",
        "No account, no tracking, no ads.",
        "Saved before the window closes — nothing gets lost.",
      ],
      leavesTitle: "Leaves only when you ask",
      leaves: [
        "AI polish — off by default, uses your own key.",
        "Device sync — optional, encrypted straight between your own computers.",
      ],
    },
    sync: {
      heading: "How local sync works.",
      lede: "Optional, off by default, and experimental. Your computers talk directly — no cloud copy exists.",
      filmLabel:
        "Posthaste sync film: two computers paired with a code, a note crossing on the same Wi-Fi, then through a relay on different networks",
      points: [
        "Pair once with a 10-minute one-time code.",
        "Connects directly on the same Wi-Fi, or passes encrypted data across networks without anyone able to read your notes.",
        "Syncs notes, steps, and projects when both computers are online.",
      ],
      phoneNote: "Phone companion app in development.",
      filmNote: "",
    },
    compare: {
      heading: "Next to the apps you already use.",
      lede: "Built for quick capture and today's plate — here is how it differs.",
      checkedPrefix: "Checked",
      pickUs: "Pick Posthaste when",
      pickOthers: "Pick the others when",
      bothNote1: "In the inbox,",
      bothNote2: "copies any note to paste elsewhere.",
    },
    free: {
      heading: "Free. MIT. No tiers.",
      body: "No subscriptions, no trial, no account. Every feature is free and open source on GitHub.",
      source: "Source on GitHub",
      license: "MIT licence",
      discord: "Join the Discord",
    },
    get: {
      heading: "Get Posthaste.",
      metaPrefix: "MIT licensed. Version",
      allFiles: "All files and release notes",
      note: "Used daily on macOS and tested on Linux. Windows installers build automatically on every release.",
      buildSummary: "Comfortable with a terminal? Build it yourself",
    },
    footer: {
      privacy: "No analytics, no cookies, no third-party requests.",
      why: "Why Posthaste",
      source: "Source on GitHub",
    },
  },
  vi: {
    nav: {
      why: "Vì sao có Posthaste",
      features: "Tính năng",
      looks: "Giao diện",
      compare: "So sánh",
      changelog: "Cập nhật",
      download: "Tải về",
    },
    hero: {
      pos: "trạng từ",
      def: "nhanh chóng; ngay lập tức.",
      whyLink: "Câu chuyện",
      title: "Ghi chú việc cần làm, nhanh như tốc độ gõ phím.",
      sub1: "Một phím tắt trên mọi ứng dụng. Quản lý ghi chú việc cần làm, rồi quay lại việc ngay.",
      sub2: "Dữ liệu hoàn toàn nằm trên máy bạn, an toàn và đồng bộ giữa các thiết bị mà không lưu trên đám mây.",
      sub3: "",
      facts: [
        "Miễn phí",
        "Tối giản",
        "Nhanh",
        "Local",
        "Tùy biến",
        "AI tùy chọn",
        "Ưu tiên bàn phím",
      ],
      downloadFor: (os) => `Tải cho ${os}`,
      downloadFree: "Tải miễn phí",
      watchDemo: "Xem video demo",
    },
    demo: {
      notes: ["Mua sữa yến mạch trên đường về nhà", "Gửi Minh link bản demo"],
    },
    moments: {
      heading: "Ghi nhanh. Xem việc hôm nay. Đánh dấu xong.",
      step1: "Ghi nhanh",
      step1Desc: "Gõ, nhấn Enter, quay lại việc.",
      step1Empty: "Thử gõ vào ô ở trên — ghi chú sẽ hiện ngay ở mục Hôm nay.",
      step1Count: (n) => `Bạn vừa ghi ${n} ghi chú trên trang này.`,
      step2: "Xem việc hôm nay",
      step3: "Đánh dấu xong",
      step3Hint: "Chọn ghi chú rồi nhấn X.",
      undo: "Hoàn tác",
    },
    features: {
      heading: "Gọn nhẹ, nhưng đủ mọi thứ bạn cần.",
      looksLink: "Thử 6 giao diện bên dưới",
      looksTail: ", hoặc nhấn phím 1–6.",
    },
    looks: {
      heading: "Màu sắc của riêng bạn.",
      lede: "Chọn một giao diện (hoặc nhấn 1–6) để đổi màu toàn trang y như ứng dụng.",
      makeOwn: "Hoặc tự tạo giao diện",
      makeOwnHelp: "Chọn 4 màu. Posthaste tự kiểm tra độ tương phản trước khi lưu.",
      share: "Chia sẻ chỉ bằng một dòng mã",
    },
    local: {
      heading: "Nằm trên máy bạn, không nằm trên máy chủ của chúng tôi.",
      staysTitle: "Luôn ở trên máy tính của bạn",
      stays: [
        "Mọi dữ liệu nằm gọn trong một tệp trên máy — dễ dàng sao lưu bất cứ đâu.",
        "Không tài khoản, không theo dõi, không quảng cáo.",
        "Lưu xong trước khi cửa sổ đóng — không bao giờ mất chữ.",
      ],
      leavesTitle: "Chỉ rời máy khi bạn tự bật",
      leaves: [
        "AI chỉnh câu chữ — tắt mặc định, dùng khóa riêng của bạn.",
        "Đồng bộ thiết bị — tùy chọn, mã hóa trực tiếp giữa các máy tính của bạn.",
      ],
    },
    sync: {
      heading: "Đồng bộ cục bộ hoạt động thế nào.",
      lede: "Tùy chọn, tắt mặc định và đang thử nghiệm. Các máy tính kết nối trực tiếp, không lưu bản sao trên đám mây.",
      filmLabel:
        "Video đồng bộ Posthaste: hai máy tính ghép đôi bằng một mã, một ghi chú đi qua cùng mạng Wi-Fi, rồi đi qua máy chuyển tiếp khi ở hai mạng khác nhau",
      points: [
        "Ghép đôi một lần bằng mã dùng một lần (hiệu lực 10 phút).",
        "Kết nối thẳng khi cùng Wi-Fi, hoặc truyền dữ liệu đã mã hóa khi khác mạng mà không ai đọc được ghi chú.",
        "Đồng bộ ghi chú, các bước và dự án khi hai máy cùng trực tuyến.",
      ],
      phoneNote: "Ứng dụng điện thoại đang được phát triển.",
      filmNote: "",
    },
    compare: {
      heading: "Đặt cạnh các ứng dụng bạn quen dùng.",
      lede: "Tập trung vào ghi nhanh và việc trong ngày — bảng so sánh nhanh bên dưới.",
      checkedPrefix: "Kiểm tra ngày",
      pickUs: "Chọn Posthaste khi",
      pickOthers: "Chọn ứng dụng khác khi",
      bothNote1: "Trong hộp thư, nhấn",
      bothNote2: "để sao chép ghi chú sang ứng dụng khác.",
    },
    free: {
      heading: "Miễn phí. Mã nguồn mở. Không gói trả phí.",
      body: "Không thuê bao, không dùng thử, không tài khoản. Mọi tính năng đều miễn phí và mở mã nguồn trên GitHub.",
      source: "Mã nguồn trên GitHub",
      license: "Giấy phép MIT",
      discord: "Tham gia Discord",
    },
    get: {
      heading: "Tải Posthaste.",
      metaPrefix: "Giấy phép MIT. Phiên bản",
      allFiles: "Tất cả tệp cài đặt & ghi chú phát hành",
      note: "Dùng hằng ngày trên macOS và kiểm thử trên Linux. Bản Windows được đóng gói tự động mỗi bản phát hành.",
      buildSummary: "Quen dùng terminal? Tự build từ mã nguồn",
    },
    footer: {
      privacy: "Không analytics, không cookie, không gọi tài nguyên ngoài.",
      why: "Vì sao có Posthaste",
      source: "Mã nguồn trên GitHub",
    },
  },
};
