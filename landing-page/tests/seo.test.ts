import { describe, expect, it } from "vitest";
import robots from "../src/app/robots";
import sitemap from "../src/app/sitemap";
import { PRODUCT } from "../src/config";
import { jsonLdString, softwareJsonLd } from "../src/lib/seo";

describe("search metadata", () => {
  it("lists the one page at the canonical address", () => {
    expect(sitemap()).toEqual([{ url: PRODUCT.siteUrl }]);
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
