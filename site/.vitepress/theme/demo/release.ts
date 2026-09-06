import { computed, ref, type Ref } from "vue";
import { inBrowser } from "vitepress";
import { PRODUCTS } from "../../products";

/**
 * One fetch of a product's latest GitHub release, shared by every component on
 * the page that wants a piece of it: the download card, the hero button and
 * the version line in the demo windows. Nothing runs during SSR.
 */
export interface Asset {
  name: string;
  browser_download_url: string;
  size: number;
}
export interface Release {
  tag_name: string;
  html_url: string;
  body: string;
  published_at: string;
  assets: Asset[];
}
interface Entry {
  release: Ref<Release | null>;
  failed: Ref<boolean>;
}

const cache = new Map<string, Entry>();

export function useLatestRelease(productId = "maplelink"): Entry {
  let entry = cache.get(productId);
  if (!entry) {
    entry = { release: ref(null), failed: ref(false) };
    cache.set(productId, entry);
    if (inBrowser) {
      const product = PRODUCTS[productId];
      fetch(`https://api.github.com/repos/${product.repo}/releases/latest`)
        .then((res) => {
          if (!res.ok) throw new Error(String(res.status));
          return res.json();
        })
        .then((json: Release) => (entry!.release.value = json))
        .catch(() => (entry!.failed.value = true));
    }
  }
  return entry;
}

/** `0.5.1` from a tag like `v0.5.1` or `v0.5.1.2608180306`; null until loaded. */
export function useLatestVersion(productId = "maplelink") {
  const { release } = useLatestRelease(productId);
  return computed(() => release.value?.tag_name.replace(/^v/, "").split(".").slice(0, 3).join(".") ?? null);
}
