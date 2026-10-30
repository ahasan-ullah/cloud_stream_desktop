import type { Repo } from "./api";

export type RootName = "home" | "search" | "library" | "downloads" | "settings";

export type SettingsPageName =
  | "settings-general"
  | "settings-player"
  | "settings-providers"
  | "settings-layout"
  | "settings-backup";

export type Route =
  | { name: RootName; query?: string }
  | { name: SettingsPageName | "extensions" }
  | { name: "plugins"; repo: Repo | null }
  | { name: "details"; provider: string; url: string; title: string; autoplay?: boolean }
  | { name: "player"; key: number };

/** Minimal stack router: the nav rail resets the stack, everything else pushes onto it. */
class Router {
  stack = $state<Route[]>([{ name: "home" }]);

  get current(): Route {
    return this.stack[this.stack.length - 1];
  }

  get rootName(): RootName {
    return this.stack[0].name as RootName;
  }

  get canGoBack() {
    return this.stack.length > 1;
  }

  push(route: Route) {
    this.stack.push(route);
  }

  /** Replaces the top entry (used for "next episode"). */
  replace(route: Route) {
    this.stack[this.stack.length - 1] = route;
  }

  back() {
    if (this.stack.length > 1) this.stack.pop();
  }

  root(name: RootName, query?: string) {
    this.stack = [{ name, query }];
  }
}

export const router = new Router();
