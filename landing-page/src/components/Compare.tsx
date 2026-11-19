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
  const others = COMPARE_APPS.slice(1);
  const rows = lang === "vi" ? COMPARE_VI.rows : COMPARE_ROWS;
  const pickUs = lang === "vi" ? COMPARE_VI.pickPosthaste : PICK_POSTHASTE;
  const pickOthers = lang === "vi" ? COMPARE_VI.pickOthers : PICK_OTHERS;

  return (
    <section className="page compare" id="compare" aria-labelledby="compare-title">
      <h2 className="h2" id="compare-title">
        {copy.compare.heading}
      </h2>
      <p className="compare__lede">{copy.compare.lede}</p>

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
