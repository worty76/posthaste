import { PRODUCT } from "@/config";

/**
 * schema.org description of the app, injected as JSON-LD. Facts only: no
 * ratings, reviews or counts (the honesty rules forbid inventing proof).
 */
export function softwareJsonLd() {
  const site = PRODUCT.siteUrl;
  return {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: PRODUCT.name,
    applicationCategory: "ProductivityApplication",
    operatingSystem: "macOS, Windows, Linux",
    description:
      "A fast, free, customizable keyboard-first note app. Press a shortcut over any app, type, press Enter. Notes stay on your computer; AI polish is optional and uses your own key.",
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    license: "https://opensource.org/licenses/MIT",
    downloadUrl: PRODUCT.releasesUrl,
    sameAs: [PRODUCT.repoUrl],
    ...(site ? { url: site, image: `${site}/og.png` } : {}),
  };
}

/** Safe to place inside a <script> element. */
export function jsonLdString(data: unknown): string {
  return JSON.stringify(data).replace(/</g, "\\u003c");
}
