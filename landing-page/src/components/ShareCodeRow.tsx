"use client";

// A real share code for the look the page is wearing, in the format the
// app imports through "Add from code…". No account, no upload.

import { useState } from "react";
import { useSite } from "./SiteState";
import { lookById } from "@/lib/looks";
import { encodeLookCode } from "@/lib/share-code";

export function ShareCodeRow() {
  const { lookId } = useSite();
  const look = lookById(lookId);
  const [copied, setCopied] = useState<string | null>(null);
  const code = encodeLookCode(look);

  return (
    <div className="share">
      <label className="share__label" htmlFor="share-code">
        {look.name}, as a code
      </label>
      <div className="share__row">
        <input
          id="share-code"
          className="share__code"
          value={code}
          readOnly
          onFocus={(e) => e.currentTarget.select()}
        />
        <button
          type="button"
          className="btn"
          onClick={() => {
            navigator.clipboard.writeText(code).then(
              () => setCopied(code),
              () => setCopied(null),
            );
          }}
        >
          {copied === code ? "Copied" : "Copy code"}
        </button>
      </div>
      <p className="share__note" aria-live="polite">
        Paste into Settings → Appearance → Add from code.
      </p>
    </div>
  );
}
