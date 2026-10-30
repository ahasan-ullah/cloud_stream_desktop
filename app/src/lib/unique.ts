/**
 * Drops repeated entries. Plugin data often lists the same title or episode twice, and a keyed
 * Svelte {#each} throws on duplicate keys, which would freeze the whole screen.
 */
export function uniqueBy<T>(items: T[], key: (item: T) => string): T[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    const k = key(item);
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
}
