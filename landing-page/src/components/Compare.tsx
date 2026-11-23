"use client";

import { useSite } from "./SiteState";
import {
  COMPARE_APPS,
  COMPARE_CHECKED_ON,
  COMPARE_ROWS,
  COMPARE_VI,
  PICK_OTHERS,
  PICK_POSTHASTE,
} from "@/lib/compare";
import { formatIsoDate as formatChecked } from "@/lib/dates";

/** How Posthaste sits next to other note and capture apps. Plain table, no script: on a
    narrow screen the table scrolls sideways inside its own region. */
export function Compare() {
  const { lang, copy } = useSite();
  const vi = lang === "vi";
  const others = COMPARE_APPS.slice(1);
  const rows = vi ? COMPARE_VI.rows : COMPARE_ROWS;
  const pickUs = vi ? COMPARE_VI.pickPosthaste : PICK_POSTHASTE;
  const pickOthers = vi ? COMPARE_VI.pickOthers : PICK_OTHERS;

  const stats = vi
    ? [
        { value: "~6 MB", label: "Bản tải về (~15 MB khi cài)" },
        { value: "~50 MB", label: "RAM khi chạy nền (4 cửa sổ sẵn sàng)" },
        { value: "0.53 ms", label: "Lưu ghi chú xuống ổ cứng" },
        { value: "0.31 ms", label: "Tìm kiếm trong 20.000 ghi chú" },
      ]
    : [
        { value: "~6 MB", label: "Download (~15 MB installed)" },
        { value: "~50 MB", label: "Idle RAM (all 4 windows warm)" },
        { value: "0.53 ms", label: "Note save commit to disk" },
        { value: "0.31 ms", label: "Search across 20,000 notes" },
      ];

  const bars = vi
    ? [
        {
          name: "Posthaste",
          sub: "Rust + WebAssembly · macOS, Windows, Linux",
          ram: "~50 MB RAM",
          disk: "~15 MB trên máy",
          pct: 11,
          us: true,
        },
        {
          name: "App gốc macOS (Things 3, Bear, Apple Notes)",
          sub: "Swift / AppKit · Chỉ có trên hệ sinh thái Apple",
          ram: "~60–180 MB RAM",
          disk: "~70–150 MB trên máy",
          pct: 28,
          us: false,
        },
        {
          name: "App Electron (Obsidian, Todoist, TickTick, Notion)",
          sub: "Đóng gói kèm trình duyệt Chromium & Node.js riêng",
          ram: "~250–600+ MB RAM",
          disk: "~150–450 MB trên máy",
          pct: 88,
          us: false,
        },
      ]
    : [
        {
          name: "Posthaste",
          sub: "Rust + WebAssembly · macOS, Windows, Linux",
          ram: "~50 MB RAM",
          disk: "~15 MB on disk",
          pct: 11,
          us: true,
        },
        {
          name: "Native macOS apps (Things 3, Bear, Apple Notes)",
          sub: "Swift / AppKit · Apple platforms only",
          ram: "~60–180 MB RAM",
          disk: "~70–150 MB on disk",
          pct: 28,
          us: false,
        },
        {
          name: "Electron apps (Obsidian, Todoist, TickTick, Notion)",
          sub: "Bundles its own Chromium browser & Node.js runtime",
          ram: "~250–600+ MB RAM",
          disk: "~150–450 MB on disk",
          pct: 88,
          us: false,
        },
      ];

  return (
    <section className="page compare" id="compare" aria-labelledby="compare-title">
      <h2 className="h2" id="compare-title">
        {copy.compare.heading}
      </h2>
      <p className="compare__lede">{copy.compare.lede}</p>

      <div
        className="bench"
        aria-label={vi ? "Hiệu năng và mức dùng bộ nhớ" : "Footprint and speed benchmark"}
      >
        <div className="bench__stats">
          {stats.map((s) => (
            <div className="bench__stat" key={s.label}>
              <span className="bench__val">{s.value}</span>
              <span className="bench__lbl">{s.label}</span>
            </div>
          ))}
        </div>

        <div className="bench__bars">
          {bars.map((b) => (
            <div className={`bench__row${b.us ? " bench__row--us" : ""}`} key={b.name}>
              <div className="bench__meta">
                <div>
                  <strong className="bench__name">{b.name}</strong>
                  <span className="bench__sub">{b.sub}</span>
                </div>
                <div className="bench__nums">
                  <span className="bench__ram">{b.ram}</span>
                  <span className="bench__disk">{b.disk}</span>
                </div>
              </div>
              <div className="bench__track" aria-hidden="true">
                <div className="bench__fill" style={{ width: `${b.pct}%` }} />
              </div>
            </div>
          ))}
        </div>
        <p className="quiet bench__foot">
          {vi
            ? "Đo trên bản phát hành v0.7.1 (macOS Apple Silicon, cơ sở dữ liệu 20.000 ghi chú)."
            : "Measured on v0.7.1 release build (macOS Apple Silicon, 20,000-note database)."}
        </p>
      </div>

      <div
        className="compare__scroll"
        role="region"
        aria-label="Comparison table. Scrolls sideways on small screens."
        tabIndex={0}
      >
        <table className="compare__table">
          <caption className="visually-hidden">
            Posthaste compared with {others.map((a) => a.name).join(", ")}, checked on{" "}
            {formatChecked(COMPARE_CHECKED_ON)}.
          </caption>
          <thead>
            <tr>
              <th scope="col">
                <span className="visually-hidden">Topic</span>
              </th>
              {COMPARE_APPS.map((app) => (
                <th
                  key={app.id}
                  scope="col"
                  className={app.id === "posthaste" ? "compare__us" : undefined}
                >
                  {app.name}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.label}>
                <th scope="row">{row.label}</th>
                {COMPARE_APPS.map((app) => (
                  <td key={app.id} className={app.id === "posthaste" ? "compare__us" : undefined}>
                    {row.cells[app.id]}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <p className="quiet compare__note">
        {copy.compare.checkedPrefix} {formatChecked(COMPARE_CHECKED_ON)}:{" "}
        {others.map((app, i) => (
          <span key={app.id}>
            <a href={app.url} rel="noopener noreferrer">
              {app.name}
            </a>
            {i < others.length - 1 ? ", " : "."}
          </span>
        ))}
      </p>

      <div className="compare__picks">
        <div>
          <h3 className="h3">{copy.compare.pickUs}</h3>
          <ul className="ledger">
            {pickUs.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
        <div>
          <h3 className="h3">{copy.compare.pickOthers}</h3>
          <ul className="ledger">
            {pickOthers.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      </div>

      <p className="quiet compare__both">
        {copy.compare.bothNote1} <kbd>C</kbd> {copy.compare.bothNote2}
      </p>
    </section>
  );
}
