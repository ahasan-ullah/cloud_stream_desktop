import { convertFileSrc } from "@tauri-apps/api/core";

// Keeps image memory bounded: an <img> only gets its src when it comes near the visible part
// of its scroll container, and loses it again once it is far away, so Chromium can drop the
// decoded bitmap. Scroll containers opt in with a `data-scroll-root` attribute.

type Pair = { near: IntersectionObserver; far: IntersectionObserver };

const observers = new Map<Element | null, Pair>();
const sources = new WeakMap<Element, string>();

function pairFor(root: Element | null): Pair {
  let pair = observers.get(root);
  if (!pair) {
    const near = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          const src = sources.get(e.target);
          if (e.isIntersecting && src && e.target.getAttribute("src") !== src) {
            (e.target as HTMLImageElement).src = src;
          }
        }
      },
      { root, rootMargin: "300px" },
    );
    const far = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (!e.isIntersecting) e.target.removeAttribute("src");
      },
      { root, rootMargin: "1600px" },
    );
    pair = { near, far };
    observers.set(root, pair);
  }
  return pair;
}

/** Attachment: `<img {@attach lazySrc(url)} />`. */
export function lazySrc(src: string | undefined) {
  return (img: HTMLImageElement) => {
    if (!src) return;
    const root = img.parentElement?.closest("[data-scroll-root]") ?? null;
    const { near, far } = pairFor(root);
    sources.set(img, src);
    near.observe(img);
    far.observe(img);
    return () => {
      near.unobserve(img);
      far.unobserve(img);
      sources.delete(img);
      img.removeAttribute("src");
      if (root && !root.isConnected) observers.delete(root);
    };
  };
}

// The `thumb:` scheme handled in src-tauri/src/thumbs.rs (http://thumb.localhost/ on Windows).
const thumbBase = (() => {
  try {
    return convertFileSrc("", "thumb");
  } catch {
    return ""; // Not running inside Tauri.
  }
})();

/**
 * URL of `url` scaled to `cssWidth` CSS pixels. Chromium keeps every image decoded at its
 * original size (a typical 1000×1500 poster is ~6 MB of bitmap), so posters are shrunk by the
 * app to what the card actually shows (~0.15 MB). TMDB links are also pointed at a smaller
 * rendition so less is downloaded.
 */
export function thumb(url: string | undefined, cssWidth: number): string | undefined {
  if (!url) return url;
  const px = Math.ceil((cssWidth * Math.min(window.devicePixelRatio || 1, 2)) / 40) * 40;
  const tmdb = px <= 185 ? 185 : px <= 342 ? 342 : 780;
  const source = url.replace(/(image\.tmdb\.org\/t\/p\/)(?:w\d+|original)\//, `$1w${tmdb}/`);
  if (!thumbBase || !/^https?:\/\//i.test(source)) return source;
  return `${thumbBase}?w=${px}&u=${encodeURIComponent(source)}`;
}
