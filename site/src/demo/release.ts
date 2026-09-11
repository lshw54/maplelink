import { useEffect, useState } from "react";
import { PRODUCTS } from "./products";

/**
 * One fetch of a product's latest GitHub release, shared by every component on
 * the page that wants a piece of it: the download card, the hero button and
 * the version line in the demo windows. Runs only in the browser.
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
export interface ReleaseState {
  release: Release | null;
  failed: boolean;
}

const cache = new Map<string, ReleaseState>();
const inflight = new Map<string, Promise<ReleaseState>>();
const listeners = new Map<string, Set<(s: ReleaseState) => void>>();

function load(productId: string): Promise<ReleaseState> {
  const hit = inflight.get(productId);
  if (hit) return hit;
  const product = PRODUCTS[productId];
  const p = fetch(`https://api.github.com/repos/${product.repo}/releases/latest`)
    .then((res) => {
      if (!res.ok) throw new Error(String(res.status));
      return res.json() as Promise<Release>;
    })
    .then((release) => ({ release, failed: false }))
    .catch(() => ({ release: null, failed: true }))
    .then((state) => {
      cache.set(productId, state);
      listeners.get(productId)?.forEach((fn) => fn(state));
      return state;
    });
  inflight.set(productId, p);
  return p;
}

const EMPTY: ReleaseState = { release: null, failed: false };

export function useLatestRelease(productId = "maplelink"): ReleaseState {
  const [state, setState] = useState<ReleaseState>(() => cache.get(productId) ?? EMPTY);
  // A changed product means the state belongs to the previous one. React's own
  // answer to that is to adjust during render rather than in an effect, which
  // also avoids the extra paint an effect would cause.
  const [shownId, setShownId] = useState(productId);
  if (shownId !== productId) {
    setShownId(productId);
    setState(cache.get(productId) ?? EMPTY);
  }

  useEffect(() => {
    // Subscribe first, so a fetch that lands between here and the render above
    // is not missed; a cache hit simply means `load` resolves immediately.
    const set = listeners.get(productId) ?? new Set();
    set.add(setState);
    listeners.set(productId, set);
    if (!cache.has(productId)) load(productId);
    return () => {
      set.delete(setState);
    };
  }, [productId]);
  return state;
}

/** `0.5.1` from a tag like `v0.5.1` or `v0.5.1.2608180306`; null until loaded. */
export function useLatestVersion(productId = "maplelink"): string | null {
  const { release } = useLatestRelease(productId);
  return release ? release.tag_name.replace(/^v/, "").split(".").slice(0, 3).join(".") : null;
}
