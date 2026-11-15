import type { Download, Platform } from "@/config";

/**
 * Which installer to offer first. Null when the guess would be wrong or
 * useless (phones, tablets, ChromeOS, ARM Linux): then the page lists all
 * three instead of pushing one.
 */
export function detectPlatform(ua: string, touchPoints = 0): Platform | null {
  if (/iPhone|iPad|iPod|Android|CrOS/i.test(ua)) return null;
  if (/Macintosh|Mac OS X/i.test(ua)) return touchPoints > 1 ? null : "mac"; // iPadOS says Macintosh
  if (/Windows/i.test(ua)) return "windows";
  if (/Linux/i.test(ua)) return /aarch64|arm/i.test(ua) ? null : "linux";
  return null;
}

export function primaryDownload(
  downloads: Record<Platform, Download | null>,
  platform: Platform | null,
): Download | null {
  return platform ? (downloads[platform] ?? null) : null;
}
