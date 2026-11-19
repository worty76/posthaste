"use client";

import { PRODUCT } from "@/config";
import { useSite } from "./SiteState";

/** No pricing table, because Posthaste is free under the MIT licence. */
export function FreeCard() {
  const { copy } = useSite();
  return (
    <section className="page free" id="free" aria-labelledby="free-title">
      <div className="free__card">
        <h2 className="h2" id="free-title">
          {copy.free.heading}
        </h2>
        <p>{copy.free.body}</p>
        <p className="free__links">
          <a href={PRODUCT.repoUrl}>{copy.free.source}</a>
          <a href={`${PRODUCT.repoUrl}/blob/main/LICENSE`}>{copy.free.license}</a>
          <a href={PRODUCT.discordUrl} rel="noopener noreferrer">
            {copy.free.discord}
          </a>
        </p>
      </div>
    </section>
  );
}
