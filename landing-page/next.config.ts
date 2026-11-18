import type { NextConfig } from "next";

// Static export: the site is plain files that any static host can serve,
// with no server logic — same privacy posture as the app.
const nextConfig: NextConfig = {
  output: "export",
  // /story/ is story/index.html, which any static server finds; /story would
  // need the host to know to try story.html.
  trailingSlash: true,
};

export default nextConfig;
