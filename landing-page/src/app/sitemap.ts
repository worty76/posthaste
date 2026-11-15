import type { MetadataRoute } from "next";
import { PRODUCT } from "@/config";

export const dynamic = "force-static";

/** One page. The URL matches the canonical link in layout.tsx exactly. */
export default function sitemap(): MetadataRoute.Sitemap {
  return PRODUCT.siteUrl ? [{ url: PRODUCT.siteUrl }] : [];
}
