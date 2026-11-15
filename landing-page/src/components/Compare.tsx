import {
  COMPARE_APPS,
  COMPARE_CHECKED_ON,
  COMPARE_ROWS,
  PICK_OTHERS,
  PICK_POSTHASTE,
  VS_TODOGLOW,
} from "@/lib/compare";

import { formatIsoDate as formatChecked } from "@/lib/dates";

/** How Posthaste sits next to other note and capture apps. Plain table, no script: on a
    narrow screen the table scrolls sideways inside its own region. */
export function Compare() {
  const others = COMPARE_APPS.slice(1);
  return (
    <section className="page compare" id="compare" aria-labelledby="compare-title">
      <h2 className="h2" id="compare-title">
        Next to the apps you already use.
      </h2>
      <p className="compare__lede">
        Posthaste catches a thought and keeps it on a short to-do list until it&apos;s done. It
        doesn&apos;t replace a notebook, so this is where it differs, and where the others are the
        better pick.
      </p>

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
            {COMPARE_ROWS.map((row) => (
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
        Checked {formatChecked(COMPARE_CHECKED_ON)}. Plans change, so check the source:{" "}
        {others.map((app, i) => (
          <span key={app.id}>
            <a href={app.url} rel="noopener noreferrer">
              {app.name}
            </a>
            {i < others.length - 1 ? ", " : "."}
          </span>
        ))}
      </p>

      <h3 className="h3 compare__vs-title">Next to TodoGlow</h3>
      <p className="compare__vs-lede">
        TodoGlow is the closest in spirit: a keyboard-first to-do app that one global shortcut
        opens. Where its page says nothing, this says it doesn&apos;t list the thing, not that the
        app lacks it.
      </p>
      <div className="compare__picks compare__picks--vs">
        <div>
          <h4 className="compare__vs-head">
            What Posthaste has that TodoGlow&apos;s page doesn&apos;t list
          </h4>
          <ul className="ledger">
            {VS_TODOGLOW.ours.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
        <div>
          <h4 className="compare__vs-head">What TodoGlow has that Posthaste doesn&apos;t</h4>
          <ul className="ledger">
            {VS_TODOGLOW.theirs.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      </div>

      <div className="compare__picks">
        <div>
          <h3 className="h3">Pick Posthaste when</h3>
          <ul className="ledger">
            {PICK_POSTHASTE.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
        <div>
          <h3 className="h3">Pick the others when</h3>
          <ul className="ledger">
            {PICK_OTHERS.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      </div>

      <p className="quiet compare__both">
        Using both works. In the inbox, <kbd>C</kbd> copies a note, ready to paste into the app you
        keep.
      </p>
    </section>
  );
}
