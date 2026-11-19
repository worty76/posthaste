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
      sub1: "Press a shortcut over any app to jot a thought down.",
      sub2: "Hit Enter and go right back to what you were doing.",
      sub3: "Private on your computer, with optional encrypted sync between your own devices.",
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
      step1Desc: "Over any app. Type, press Enter, and you're back.",
      step1Empty: "Try the box at the top: notes you save there land in Today.",
      step1Count: (n) => `You caught ${n} ${n === 1 ? "note" : "notes"} on this page.`,
      step2: "See what's left",
      step3: "Tick it off",
      step3Hint: "Select a note in Today and press X.",
      undo: "Undo",
    },
    features: {
      heading: "Small, and it does all of this.",
      looksLink: "Try the six looks below",
      looksTail: ", or press 1 to 6 anywhere on this page.",
    },
    looks: {
      heading: "Your colors, on all of it.",
      lede: "Pick a look and this page changes the way the app does: colors, corners, even the shape of the letters.",
      makeOwn: "Or make your own",
      makeOwnHelp: "Four colors. If text would be hard to read, Posthaste tells you before saving.",
      share: "Share it as one line",
    },
    local: {
      heading: "On your computer, not ours.",
      staysTitle: "Stays on your computer",
      stays: [
        "All your notes, drafts and looks live in one file you can back up anywhere.",
        "No account. No tracking. No ads.",
        "Your note is saved before the window closes — nothing gets lost.",
      ],
      leavesTitle: "Leaves only when you ask",
      leaves: [
        "AI polish — off by default, uses your own key.",
        "Device sync — optional and encrypted straight between your own computers.",
      ],
    },
    sync: {
      heading: "How local sync works.",
      lede: "Sync is optional, off by default and still experimental. Your computers talk to each other, and no cloud copy of your notes exists.",
      filmLabel:
        "Posthaste sync film: two computers paired with a code, a note crossing on the same Wi-Fi, then through a relay on different networks",
      points: [
        "Pair once. One computer makes a single-use code that works for 10 minutes, and the other computer pastes it.",
        "On the same Wi-Fi, the two computers connect straight to each other.",
        "On different networks, when they can't connect directly, the encrypted traffic goes through a relay that only passes it along. It sees device IDs and IP addresses, never your notes. Leave the relay address empty for same-network sync only.",
        "Both computers have to be on at the same time. Nothing waits for the other one in the cloud.",
        "Notes sync today. Attachments, looks, settings and unsaved drafts don't yet.",
      ],
      phoneNote: "A phone app is in development. Today, sync is between computers.",
      filmNote:
        "Built from mock-ups of the app's windows, not a screen recording. The sound is made up for the film; the app itself is silent.",
    },
    compare: {
      heading: "Next to the apps you already use.",
      lede: "Posthaste catches a quick thought and keeps today's list short. Here is how it compares.",
      checkedPrefix: "Checked",
      pickUs: "Pick Posthaste when",
      pickOthers: "Pick the others when",
      bothNote1: "Using both works. In the inbox,",
      bothNote2: "copies a note, ready to paste anywhere.",
    },
    free: {
      heading: "Free. MIT. No tiers.",
      body: "No plans, no trial, no account. Every feature is in the free app, and the source code is open on GitHub.",
      source: "Source on GitHub",
      license: "MIT licence",
      discord: "Join the Discord",
    },
    get: {
      heading: "Get Posthaste.",
      metaPrefix: "MIT licensed. Version",
      allFiles: "All files and release notes",
      note: "Used every day on macOS and tested on Linux. The Windows installer is built automatically and still getting polish.",
      buildSummary: "Comfortable with a terminal? Build it yourself",
    },
    footer: {
      privacy: "This page has no analytics, no cookies, and asks no other site for anything.",
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
      sub1: "Nhấn phím tắt ngay trên mọi ứng dụng để ghi lại ý nghĩ.",
      sub2: "Nhấn Enter để lưu và quay lại công việc ngay lập tức.",
      sub3: "Lưu riêng tư trên máy của bạn, có thể đồng bộ mã hóa giữa các máy tính của chính bạn.",
      facts: ["Miễn phí", "Tối giản", "Nhanh", "Local", "Tùy biến", "AI tùy chọn", "Ưu tiên bàn phím"],
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
      step1Desc: "Mở trên mọi ứng dụng. Gõ, nhấn Enter, rồi quay lại việc đang làm.",
      step1Empty: "Thử gõ vào ô ở trên: ghi chú bạn lưu sẽ hiện ngay ở mục Hôm nay.",
      step1Count: (n) => `Bạn vừa ghi ${n} ghi chú trên trang này.`,
      step2: "Xem việc hôm nay",
      step3: "Đánh dấu xong",
      step3Hint: "Chọn một ghi chú ở mục Hôm nay rồi nhấn X.",
      undo: "Hoàn tác",
    },
    features: {
      heading: "Gọn nhẹ, nhưng đủ mọi thứ bạn cần.",
      looksLink: "Thử 6 giao diện bên dưới",
      looksTail: ", hoặc nhấn phím 1 đến 6 ngay trên trang này.",
    },
    looks: {
      heading: "Màu sắc của riêng bạn.",
      lede: "Chọn một giao diện và cả trang web này sẽ đổi màu, bo góc và phông chữ y như trong ứng dụng.",
      makeOwn: "Hoặc tự tạo giao diện",
      makeOwnHelp: "Chỉ cần 4 màu. Nếu chữ khó đọc, Posthaste sẽ báo ngay trước khi lưu.",
      share: "Chia sẻ chỉ bằng một dòng mã",
    },
    local: {
      heading: "Nằm trên máy bạn, không nằm trên máy chủ của chúng tôi.",
      staysTitle: "Luôn ở trên máy tính của bạn",
      stays: [
        "Mọi ghi chú, bản nháp và giao diện nằm gọn trong một tệp để bạn tự sao lưu.",
        "Không cần tài khoản. Không theo dõi. Không quảng cáo.",
        "Ghi chú được lưu xong trước khi cửa sổ đóng — không bao giờ mất chữ.",
      ],
      leavesTitle: "Chỉ rời máy khi bạn tự bật",
      leaves: [
        "AI chỉnh sửa câu chữ — tắt mặc định, dùng khóa riêng của bạn.",
        "Đồng bộ thiết bị — tùy chọn, mã hóa trực tiếp giữa các máy tính của bạn.",
      ],
    },
    sync: {
      heading: "Đồng bộ cục bộ hoạt động thế nào.",
      lede: "Đồng bộ là tùy chọn, tắt mặc định và vẫn đang thử nghiệm. Các máy tính của bạn nói chuyện trực tiếp với nhau, không có bản sao ghi chú nào trên đám mây.",
      filmLabel:
        "Video đồng bộ Posthaste: hai máy tính ghép đôi bằng một mã, một ghi chú đi qua cùng mạng Wi-Fi, rồi đi qua máy chuyển tiếp khi ở hai mạng khác nhau",
      points: [
        "Ghép đôi một lần. Một máy tạo mã dùng một lần, có hiệu lực 10 phút, máy còn lại dán mã vào.",
        "Cùng mạng Wi-Fi, hai máy kết nối thẳng với nhau.",
        "Khác mạng, khi không kết nối thẳng được, dữ liệu đã mã hóa đi qua một máy chuyển tiếp chỉ làm nhiệm vụ chuyển đi. Máy này chỉ thấy mã thiết bị và địa chỉ IP, không bao giờ thấy ghi chú của bạn. Để trống địa chỉ này nếu chỉ muốn đồng bộ trong cùng mạng.",
        "Hai máy phải bật cùng lúc. Không có gì được giữ chờ trên đám mây.",
        "Hiện ghi chú đã đồng bộ. Tệp đính kèm, giao diện, cài đặt và bản nháp chưa lưu thì chưa.",
      ],
      phoneNote: "App điện thoại đang được phát triển. Hiện tại, đồng bộ diễn ra giữa các máy tính.",
      filmNote:
        "Làm từ hình dựng lại các cửa sổ của ứng dụng, không phải quay màn hình. Âm thanh chỉ làm cho video; bản thân ứng dụng không phát tiếng.",
    },
    compare: {
      heading: "Đặt cạnh các ứng dụng bạn quen dùng.",
      lede: "Posthaste giúp ghi nhanh và giữ danh sách việc hôm nay thật gọn. So sánh nhanh bên dưới.",
      checkedPrefix: "Kiểm tra ngày",
      pickUs: "Chọn Posthaste khi",
      pickOthers: "Chọn ứng dụng khác khi",
      bothNote1: "Dùng song song cũng rất tiện. Trong hộp thư, nhấn",
      bothNote2: "để sao chép và dán sang bất kỳ ứng dụng nào.",
    },
    free: {
      heading: "Miễn phí. Mã nguồn mở. Không gói trả phí.",
      body: "Không giới hạn, không dùng thử, không tài khoản. Mọi tính năng đều miễn phí và mã nguồn mở trên GitHub.",
      source: "Mã nguồn trên GitHub",
      license: "Giấy phép MIT",
      discord: "Tham gia Discord",
    },
    get: {
      heading: "Tải Posthaste.",
      metaPrefix: "Giấy phép MIT. Phiên bản",
      allFiles: "Tất cả tệp cài đặt & ghi chú phát hành",
      note: "Dùng hằng ngày trên macOS và đã kiểm thử trên Linux. Bản Windows được đóng gói tự động và đang tiếp tục hoàn thiện.",
      buildSummary: "Quen dùng terminal? Tự build từ mã nguồn",
    },
    footer: {
      privacy: "Trang web này không dùng analytics, không cookie và không gọi tài nguyên ngoài.",
      why: "Vì sao có Posthaste",
      source: "Mã nguồn trên GitHub",
    },
  },
};
