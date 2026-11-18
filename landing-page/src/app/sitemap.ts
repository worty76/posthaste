import type { MetadataRoute } from "next";
import { PRODUCT } from "@/config";
import { STORY } from "@/lib/story";

export const dynamic = "force-static";

/** The home page, plus the story once it is published. Each URL matches the
    canonical link of its page exactly. */
export default function sitemap(): MetadataRoute.Sitemap {
  if (!PRODUCT.siteUrl) return [];
  const pages = [{ url: PRODUCT.siteUrl }];
  if (STORY.published) pages.push({ url: `${PRODUCT.siteUrl}/story/` });
  return pages;
}
