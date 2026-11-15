import type { Metadata, Viewport } from "next";
import localFont from "next/font/local";
import { PRODUCT } from "@/config";
import { themeCss } from "@/lib/theme";
import { defaultLookId, lookById } from "@/lib/looks";
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

const TITLE = "Posthaste: get a thought out of your head in two seconds";
const DESCRIPTION =
  "Press a shortcut anywhere, type, press Enter. The window is gone and you are back where you were. Free, MIT licensed, notes stay on your computer.";

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
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: lookById(defaultLookId(false)).colors.bg },
    { media: "(prefers-color-scheme: dark)", color: lookById(defaultLookId(true)).colors.bg },
  ],
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={recursive.variable}>
      <head>
        {/* Every look's tokens, generated from lib/looks.ts at build time. */}
        <style dangerouslySetInnerHTML={{ __html: themeCss() }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
