/**
 * Minimal, dependency-free glob matcher supporting `*` and `**`.
 *
 * - `**` matches any sequence of characters, including `/` (zero or more
 *   path segments).
 * - `*` matches any sequence of characters except `/` (within one segment).
 *
 * This is intentionally small: it only needs to support the freeze_paths
 * patterns we author ourselves in ENGAGEMENT.md, not arbitrary user input.
 */

function globToRegExp(pattern: string): RegExp {
  // Escape regex-special characters, then re-introduce `*`/`**` semantics
  // via placeholder tokens so the escaping pass doesn't touch them.
  const GLOBSTAR = '\u0000GLOBSTAR\u0000';
  const STAR = '\u0000STAR\u0000';

  const withPlaceholders = pattern.replace(/\*\*/g, GLOBSTAR).replace(/\*/g, STAR);

  const escaped = withPlaceholders.replace(/[.+^${}()|[\]\\]/g, '\\$&');

  const withRegex = escaped.replaceAll(GLOBSTAR, '.*').replaceAll(STAR, '[^/]*');

  return new RegExp(`^${withRegex}$`);
}

export function matchesGlob(filePath: string, pattern: string): boolean {
  return globToRegExp(pattern).test(filePath);
}

export function matchesAnyGlob(filePath: string, patterns: readonly string[]): boolean {
  return patterns.some((pattern) => matchesGlob(filePath, pattern));
}
