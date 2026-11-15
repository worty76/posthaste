import { PRODUCT } from "@/config";

/** No pricing table, because there is no price. */
export function FreeCard() {
  return (
    <section className="page free" id="free" aria-labelledby="free-title">
      <div className="free__card">
        <h2 className="h2" id="free-title">
          Free. MIT. No tiers.
        </h2>
        <p>
          No plans, no trial, no account. Every feature is in the free app, and the code is on
          GitHub under the MIT licence, so you can read it, change it and build it yourself.
        </p>
        <p className="free__links">
          <a href={PRODUCT.repoUrl}>Source on GitHub</a>
          <a href={`${PRODUCT.repoUrl}/blob/main/LICENSE`}>MIT licence</a>
          <a href={PRODUCT.discordUrl} rel="noopener noreferrer">
            Join the Discord
          </a>
        </p>
      </div>
    </section>
  );
}
