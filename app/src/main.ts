import flagsFont from "country-flag-emoji-polyfill/dist/TwemojiCountryFlags.woff2?url";
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

// Windows' emoji font has no country flags; register Twemoji's flag glyphs (bundled, not from a CDN)
// so language flags render like on Android. Only regional-indicator code points use this font.
const flags = new FontFace("Twemoji Country Flags", `url(${flagsFont}) format("woff2")`, {
  unicodeRange: "U+1F1E6-1F1FF, U+1F3F4, U+E0062-E007F",
  display: "swap",
});
document.fonts.add(flags);
flags.load().catch(() => {});

export default mount(App, { target: document.getElementById("app")! });
