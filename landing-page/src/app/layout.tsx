import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { PRODUCT } from "@/config";
import { Analytics } from "@/components/Analytics";
import { themeCss } from "@/lib/theme";
import { DEFAULT_LOOK_ID, lookById } from "@/lib/looks";
import { jsonLdString, softwareJsonLd } from "@/lib/seo";
import "./globals.css";

// One self-hosted, open-licence variable font (licence in src/fonts/).
// Recursive's casual and mono axes stand in for the app's font modes, so a
// look changes the type's personality as well as its colours. Subset to
// Latin and the key symbols, weights 450 to 800, slant -12 to 0.
const recursive = localFont({
  src: "../fonts/recursive-latin-var.woff2",
  variable: "--font-recursive",
  weight: "450 800",
  display: "swap",
});

const TITLE =
  "Posthaste | Free, fast, local-first, keyboard-first, customizable todo note app with AI you switch on yourself";
const DESCRIPTION =
  "Press a shortcut anywhere, type, press Enter, and you are back where you were. Fast, free and customizable, with AI polish you turn on yourself using your own key. Notes stay on your computer.";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  openGraph: { title: TITLE, description: DESCRIPTION, type: "website", images: ["/og.png"] },
  twitter: {
    card: "summary_large_image",
    title: TITLE,
    description: DESCRIPTION,
    images: ["/og.png"],
  },
  icons: { icon: "/logo-256.png?v=2" },
  ...(PRODUCT.siteUrl
    ? { metadataBase: new URL(PRODUCT.siteUrl), alternates: { canonical: "/" } }
    : {}),
};

export const viewport: Viewport = {
  themeColor: lookById(DEFAULT_LOOK_ID).colors.bg,
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={recursive.variable}>
      <head>
        {/* Every look's tokens, generated from lib/looks.ts at build time. */}
        <style dangerouslySetInnerHTML={{ __html: themeCss() }} />
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: jsonLdString(softwareJsonLd()) }}
        />
      </head>
      <body>
        {children}
        <Analytics />
      </body>
    </html>
  );
}
