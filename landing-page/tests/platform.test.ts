import { describe, expect, it } from "vitest";
import { PRODUCT } from "../src/config";
import { detectPlatform, primaryDownload } from "../src/lib/platform";

const UA = {
  mac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15",
  win: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/130.0 Safari/537.36",
  linux: "Mozilla/5.0 (X11; Linux x86_64) Gecko/20100101 Firefox/132.0",
  linuxArm: "Mozilla/5.0 (X11; Linux aarch64) Gecko/20100101 Firefox/132.0",
  iphone: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Safari/604.1",
  android: "Mozilla/5.0 (Linux; Android 15; Pixel 9) Chrome/130.0 Mobile Safari/537.36",
  chromeos: "Mozilla/5.0 (X11; CrOS x86_64 15000.0.0) Chrome/130.0 Safari/537.36",
};

describe("platform detection", () => {
  it("maps desktop user agents", () => {
    expect(detectPlatform(UA.mac)).toBe("mac");
    expect(detectPlatform(UA.win)).toBe("windows");
    expect(detectPlatform(UA.linux)).toBe("linux");
  });

  it("refuses to guess for phones, tablets, ChromeOS and ARM Linux", () => {
    for (const ua of [UA.iphone, UA.android, UA.chromeos, UA.linuxArm, "curl/8"]) {
      expect(detectPlatform(ua), ua).toBeNull();
    }
    expect(detectPlatform(UA.mac, 5)).toBeNull(); // iPadOS reports Macintosh
  });

  it("offers no primary installer when the platform is unknown", () => {
    expect(primaryDownload(PRODUCT.downloads, null)).toBeNull();
    expect(primaryDownload(PRODUCT.downloads, "mac")?.url).toMatch(/\.dmg$/);
  });
});
