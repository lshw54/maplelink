/**
 * The launchers this site covers.
 *
 * Both projects publish the same shape of release on GitHub: a portable exe
 * plus a package for first-time users, with the portable exe's SHA256 in the
 * release notes. The site never hosts a file; every download button resolves
 * to a GitHub asset of the product it belongs to.
 */
export interface Product {
  id: string;
  name: string;
  /** GitHub `owner/repo`. */
  repo: string;
  /** Matches the asset recommended to first-time users. */
  setup: RegExp;
  /** Matches the bare program, the file auto-update replaces. */
  portable: RegExp;
  /** Captures the portable exe's SHA256 from the release body, group 1. */
  sha256: RegExp;
}

export const PRODUCTS: Record<string, Product> = {
  maplelink: {
    id: "maplelink",
    name: "MapleLink",
    repo: "lshw54/maplelink",
    setup: /^MapleLink-Setup\.exe$/i,
    portable: /^MapleLink\.exe$/i,
    sha256: /\|\s*SHA256\s*\|\s*`?([0-9a-f]{64})`?/i,
  },
  beanfun: {
    id: "beanfun",
    name: "Beanfun",
    repo: "pungin/Beanfun",
    setup: /^Beanfun_.*-setup\.exe$/i,
    portable: /^Beanfun\.exe$/i,
    sha256: /\|\s*SHA256\s*\|\s*`?([0-9a-f]{64})`?/i,
  },
};

export const releasesUrl = (p: Product) => `https://github.com/${p.repo}/releases`;
export const latestUrl = (p: Product) => `${releasesUrl(p)}/latest`;
