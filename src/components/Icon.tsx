const PATHS = {
  inbox:
    "M2.500 9.500 4.200 3.500h7.600l1.700 6M2.500 9.500v3h11v-3M2.500 9.500h3l.8 1.500h3.400l.8-1.500h3",
  sun: "M8 5.200a2.800 2.800 0 1 0 0 5.600 2.800 2.800 0 0 0 0-5.600ZM8 1.500v1.300M8 13.200v1.300M1.500 8h1.300M13.200 8h1.300M3.400 3.400l.9.900M11.700 11.700l.9.900M3.400 12.600l.9-.9M11.700 4.300l.9-.9",
  archive: "M2.500 3h11v3h-11zM3.500 6v7h9V6M6.500 8.800h3",
  later: "M4.500 2.500h7v11L8 10.800l-3.500 2.700z",
  settings: "M2.500 4.500h6M11.500 4.500h2M2.500 11.500h2M7.500 11.500h6M8.500 3v3M5.500 10v3",
  filter: "M2.500 3.500h11l-4.200 5v4l-2.600-1.200V8.500z",
  search: "M7 2.800a4.200 4.200 0 1 0 0 8.400 4.200 4.200 0 0 0 0-8.400ZM10.200 10.200l3.300 3.300",
  close: "M4 4l8 8M12 4l-8 8",
  prev: "M10 3.500 5.500 8l4.500 4.500",
  next: "M6 3.500 10.500 8 6 12.500",
  calendar: "M2.500 4h11v9.500h-11zM2.500 6.800h11M5.500 2.500v3M10.500 2.500v3",
  check: "M3.500 8.300l3 3 6-6.600",
  edit: "M3 13l.6-3L10.800 2.800l2.400 2.400L6 12.400 3 13Z",
  copy: "M5.500 5.500h7v7h-7zM3.500 10.500v-7h7",
  trash: "M3 4.500h10M6.500 4.500V3h3v1.500M4.500 4.500l.6 8.500h5.800l.6-8.500",
  restore: "M3 8a5 5 0 1 0 1.600-3.700M3 3v2.800h2.800",
  plus: "M8 3v10M3 8h10",
  paperclip:
    "M10.500 5.500 6.200 9.800a1.300 1.300 0 0 0 1.800 1.800l4.600-4.600a2.600 2.600 0 0 0-3.700-3.700L4.300 7.900a3.900 3.900 0 0 0 5.500 5.500l3.200-3.200",
  lines: "M3 4.500h10M3 8h10M3 11.500h6",
  file: "M4 2.500h5l3 3v8H4zM9 2.500v3h3",
  image: "M2.500 3.500h11v9h-11zM2.500 10.500l3-3 3 3 2-2 3 3M10.200 6.300h.1",
  open: "M9 2.500h4.500V7M13.500 2.500 8 8M11.500 9.500v4h-9v-9h4",
  download: "M8 2.500v8M4.800 7.500 8 10.700l3.200-3.200M3 13.500h10",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
