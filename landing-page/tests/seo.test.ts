import { describe, expect, it } from "vitest";
import robots from "../src/app/robots";
import sitemap from "../src/app/sitemap";
import { PRODUCT } from "../src/config";
import { jsonLdString, softwareJsonLd } from "../src/lib/seo";
import { STORY } from "../src/lib/story";

describe("search metadata", () => {
  it("lists the home page at the canonical address, and the story once published", () => {
    const urls = sitemap().map((p) => p.url);
    expect(urls[0]).toBe(PRODUCT.siteUrl);
    expect(urls.includes(`${PRODUCT.siteUrl}/story/`)).toBe(STORY.published);
  });

  it("allows crawling and points at the sitemap", () => {
    const r = robots();
    expect(r.rules).toEqual({ userAgent: "*", allow: "/" });
    expect(r.sitemap).toBe(`${PRODUCT.siteUrl}/sitemap.xml`);
  });

  it("describes a free app without invented proof", () => {
    const ld = softwareJsonLd() as Record<string, unknown>;
    expect(ld["@type"]).toBe("SoftwareApplication");
    expect(ld.offers).toMatchObject({ price: "0" });
    for (const key of ["aggregateRating", "review", "ratingValue", "interactionStatistic"]) {
      expect(JSON.stringify(ld)).not.toContain(key);
    }
  });

  it("cannot break out of the script element", () => {
    expect(jsonLdString({ a: "</script><b>" })).not.toContain("</script>");
  });
});
