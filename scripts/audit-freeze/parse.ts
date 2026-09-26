/**
 * Parses the small, fixed set of front-matter fields audit-freeze.yml needs
 * out of audit-prep/ENGAGEMENT.md: `freeze_paths` (a YAML list) and
 * `freeze_until` (an ISO-8601 timestamp, or the literal placeholder "TBD"
 * meaning "no freeze is active yet").
 *
 * This is a narrow, purpose-built parser -- not a general YAML parser --
 * because the front matter is authored entirely by us in a fixed shape.
 * Keeping it dependency-free avoids pulling a YAML library into a security
 * gate whose correctness we want to be easy to audit at a glance.
 */

export interface EngagementFrontMatter {
  freezePaths: string[];
  /** null when there is no active freeze (missing, unparseable, or "TBD"). */
  freezeUntil: Date | null;
}

const FRONT_MATTER_DELIMITER = /^---\s*$/;

export function parseFrontMatter(fileContent: string): EngagementFrontMatter {
  const lines = fileContent.split(/\r?\n/);

  if (!FRONT_MATTER_DELIMITER.test(lines[0] ?? '')) {
    return { freezePaths: [], freezeUntil: null };
  }

  const endIndex = lines.findIndex((line, index) => index > 0 && FRONT_MATTER_DELIMITER.test(line));
  if (endIndex === -1) {
    return { freezePaths: [], freezeUntil: null };
  }

  const frontMatterLines = lines.slice(1, endIndex);

  const freezePaths = extractListValue(frontMatterLines, 'freeze_paths');
  const freezeUntilRaw = extractScalarValue(frontMatterLines, 'freeze_until');
  const freezeUntil = parseFreezeUntil(freezeUntilRaw);

  return { freezePaths, freezeUntil };
}

function extractListValue(lines: string[], key: string): string[] {
  const keyIndex = lines.findIndex((line) => new RegExp(`^${key}:\\s*$`).test(line.trim()));
  if (keyIndex === -1) return [];

  const values: string[] = [];
  for (let i = keyIndex + 1; i < lines.length; i++) {
    const line = lines[i];
    const match = /^\s*-\s*(.+?)\s*$/.exec(line);
    if (!match) break;
    values.push(stripQuotes(match[1]));
  }
  return values;
}

function extractScalarValue(lines: string[], key: string): string | null {
  const line = lines.find((l) => new RegExp(`^${key}:\\s*.+$`).test(l.trim()));
  if (!line) return null;
  const match = new RegExp(`^${key}:\\s*(.+?)\\s*$`).exec(line.trim());
  if (!match) return null;
  return stripQuotes(match[1]);
}

function stripQuotes(value: string): string {
  if (
    (value.startsWith('"') && value.endsWith('"')) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1);
  }
  return value;
}

function parseFreezeUntil(raw: string | null): Date | null {
  if (!raw) return null;
  if (raw.trim().toUpperCase() === 'TBD') return null;

  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return null;

  return parsed;
}
