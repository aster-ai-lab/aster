/**
 * Closed front matter for apps/web/public/tenant/<slug>/INFO.md.
 * Pure: no filesystem, no env, no app imports.
 */

export type TenantInfo = {
  version: 1;
  slug: string;
  name: { zh: string; en: string };
  logos: { light_surface: string; dark_surface: string };
  /** Desktop product name. Same in both locales. */
  product: string;
  /** Model-service provider name. Omitted when this tenant has no override. */
  model_service?: string;
  brand: string;
  on_brand: string;
  theme_color: { light: string; dark: string };
  description: { zh: string; en: string };
  body: string;
};

const SLUG_RE = /^[a-z][a-z0-9-]{0,31}$/;
const LOGO_RE = /^[a-z0-9][a-z0-9.-]{0,63}$/;
const COLOR_RE = /^#[0-9A-Fa-f]{6}$/;

const PARENTS = new Set(['name', 'logos', 'theme_color', 'description']);
const SCALARS = new Set(['version', 'slug', 'product', 'brand', 'on_brand', 'model_service']);
const CHILDREN: Record<string, Set<string>> = {
  name: new Set(['zh', 'en']),
  logos: new Set(['light_surface', 'dark_surface']),
  theme_color: new Set(['light', 'dark']),
  description: new Set(['zh', 'en']),
};

const LEAVES = [
  'version',
  'slug',
  'name.zh',
  'name.en',
  'logos.light_surface',
  'logos.dark_surface',
  'product',
  'brand',
  'on_brand',
  'theme_color.light',
  'theme_color.dark',
  'description.zh',
  'description.en',
] as const;

function fail(field: string, reason: string): never {
  throw new Error(`INFO.md field ${field}: ${reason}`);
}

function codePoints(value: string): number {
  let count = 0;
  for (const _ of value) count += 1;
  return count;
}

function rejectMarkup(field: string, value: string) {
  if (value.includes('<') || value.includes('>') || value.includes('&')) {
    fail(field, 'rejects <, >, and &');
  }
  if (value.includes('\n') || value.includes('\r')) {
    fail(field, 'rejects a newline');
  }
}

function parseQuoted(field: string, raw: string): string {
  if (!raw.startsWith('"') || !raw.endsWith('"') || raw.length < 2) {
    fail(field, 'must be a double-quoted string');
  }
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    fail(field, 'must be a JSON string');
  }
  if (typeof value !== 'string') fail(field, 'must be a JSON string');
  return value;
}

function rejectYamlSyntax(line: string) {
  let quoted = false;
  for (const ch of line) {
    if (ch === '"') {
      quoted = !quoted;
      continue;
    }
    if (quoted) continue;
    if ('#{}[]|>*&!'.includes(ch)) fail('front-matter', 'rejects YAML syntax');
  }
}

function parseLine(line: string): { key: string; value: string | null } {
  const colon = line.indexOf(':');
  if (colon <= 0) fail('front-matter', `invalid line ${JSON.stringify(line)}`);
  const key = line.slice(0, colon);
  if (!/^[a-z_]+$/.test(key)) fail(key, 'invalid key');
  const rest = line.slice(colon + 1);
  if (rest === '') return { key, value: null };
  if (!rest.startsWith(' ')) fail(key, 'missing space after colon');
  const value = rest.slice(1);
  if (value === '') fail(key, 'missing value');
  return { key, value };
}

export function parseTenantInfo(text: string): TenantInfo {
  if (text.includes('\0')) fail('front-matter', 'rejects NUL');
  const lines = text.split('\n');
  if (lines[0] !== '---') fail('front-matter', 'must start with ---');
  let end = -1;
  for (let i = 1; i < lines.length; i += 1) {
    if (lines[i] === '---') {
      end = i;
      break;
    }
  }
  if (end < 0) fail('front-matter', 'missing closing ---');

  const values = new Map<string, string>();
  const seenParents = new Set<string>();
  let parent: string | null = null;

  for (let i = 1; i < end; i += 1) {
    const line = lines[i] ?? '';
    if (line === '') continue;
    if (line.includes('\t')) fail('front-matter', 'rejects tabs');
    rejectYamlSyntax(line);
    if (line.startsWith(' ')) {
      if (!line.startsWith('  ') || line.startsWith('   ')) {
        fail('front-matter', 'child indent must be two spaces');
      }
      if (!parent) fail('front-matter', 'child without a parent');
      const parsed = parseLine(line.slice(2));
      const allowed = CHILDREN[parent];
      if (!allowed || !allowed.has(parsed.key) || parsed.value === null) {
        fail(`${parent}.${parsed.key}`, 'unknown key');
      }
      const full = `${parent}.${parsed.key}`;
      if (values.has(full)) fail(full, 'duplicate key');
      values.set(full, parsed.value);
      continue;
    }
    const parsed = parseLine(line);
    if (parsed.value === null) {
      if (!PARENTS.has(parsed.key)) fail(parsed.key, 'unknown key');
      if (seenParents.has(parsed.key)) fail(parsed.key, 'duplicate key');
      seenParents.add(parsed.key);
      parent = parsed.key;
      continue;
    }
    parent = null;
    if (!SCALARS.has(parsed.key)) fail(parsed.key, 'unknown key');
    if (values.has(parsed.key)) fail(parsed.key, 'duplicate key');
    values.set(parsed.key, parsed.value);
  }

  for (const leaf of LEAVES) {
    if (!values.has(leaf)) fail(leaf, 'missing key');
  }

  const version = values.get('version') ?? '';
  if (version !== '1') fail('version', 'must be the unquoted integer 1');

  const slug = values.get('slug') ?? '';
  if (!SLUG_RE.test(slug)) fail('slug', 'must match ^[a-z][a-z0-9-]{0,31}$');

  const logos = {
    light_surface: readLogo('logos.light_surface', values.get('logos.light_surface') ?? ''),
    dark_surface: readLogo('logos.dark_surface', values.get('logos.dark_surface') ?? ''),
  };

  const name = {
    zh: readText('name.zh', values.get('name.zh') ?? '', 64),
    en: readText('name.en', values.get('name.en') ?? '', 64),
  };
  const product = readText('product', values.get('product') ?? '', 64);
  const modelService = values.has('model_service')
    ? readText('model_service', values.get('model_service') ?? '', 64)
    : undefined;
  const description = {
    zh: readText('description.zh', values.get('description.zh') ?? '', 120),
    en: readText('description.en', values.get('description.en') ?? '', 120),
  };

  const body = lines.slice(end + 1).join('\n').trim();
  if (!body) fail('body', 'must be non-empty');

  return {
    version: 1,
    slug,
    name,
    logos,
    product,
    ...(modelService ? { model_service: modelService } : {}),
    brand: readColor('brand', values.get('brand') ?? ''),
    on_brand: readColor('on_brand', values.get('on_brand') ?? ''),
    theme_color: {
      light: readColor('theme_color.light', values.get('theme_color.light') ?? ''),
      dark: readColor('theme_color.dark', values.get('theme_color.dark') ?? ''),
    },
    description,
    body,
  };
}

function readLogo(field: string, raw: string): string {
  if (raw.startsWith('"') || raw.includes('/') || raw.includes('..')) {
    fail(field, 'must be a single png filename');
  }
  if (!LOGO_RE.test(raw) || !raw.endsWith('.png')) {
    fail(field, 'must be a png filename');
  }
  return raw;
}

function readText(field: string, raw: string, max: number): string {
  const value = parseQuoted(field, raw).trim();
  rejectMarkup(field, value);
  const count = codePoints(value);
  if (count < 1 || count > max) fail(field, `must be 1–${max} code points`);
  return value;
}

function readColor(field: string, raw: string): string {
  const value = parseQuoted(field, raw);
  if (!COLOR_RE.test(value)) fail(field, 'must be #RRGGBB');
  return value;
}

function quote(value: string): string {
  return JSON.stringify(value);
}

/** Canonical INFO.md. Round-trips through parseTenantInfo. */
export function serializeTenantInfo(info: TenantInfo): string {
  if (info.version !== 1) fail('version', 'must be the unquoted integer 1');
  const body = info.body.trim();
  const text = [
    '---',
    'version: 1',
    `slug: ${info.slug.trim()}`,
    'name:',
    `  zh: ${quote(info.name.zh.trim())}`,
    `  en: ${quote(info.name.en.trim())}`,
    `product: ${quote(info.product.trim())}`,
    ...(info.model_service ? [`model_service: ${quote(info.model_service.trim())}`] : []),
    'logos:',
    `  light_surface: ${info.logos.light_surface.trim()}`,
    `  dark_surface: ${info.logos.dark_surface.trim()}`,
    `brand: ${quote(info.brand.trim())}`,
    `on_brand: ${quote(info.on_brand.trim())}`,
    'theme_color:',
    `  light: ${quote(info.theme_color.light.trim())}`,
    `  dark: ${quote(info.theme_color.dark.trim())}`,
    'description:',
    `  zh: ${quote(info.description.zh.trim())}`,
    `  en: ${quote(info.description.en.trim())}`,
    '---',
    '',
    body,
    '',
  ].join('\n');
  parseTenantInfo(text);
  return text;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function asString(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

/** Closed record check. Extra keys are dropped; missing or invalid leaves fail in the parser. */
export function tenantInfoFromRecord(value: unknown): TenantInfo {
  const record = asRecord(value);
  const name = asRecord(record?.name);
  const logos = asRecord(record?.logos);
  const theme = asRecord(record?.theme_color);
  const description = asRecord(record?.description);
  if (!record || !name || !logos || !theme || !description) {
    fail('front-matter', 'invalid record');
  }
  if (record.version !== 1) fail('version', 'must be the unquoted integer 1');
  return parseTenantInfo(
    serializeTenantInfo({
      version: 1,
      slug: asString(record.slug),
      name: { zh: asString(name.zh), en: asString(name.en) },
      logos: {
        light_surface: asString(logos.light_surface),
        dark_surface: asString(logos.dark_surface),
      },
      product: asString(record.product),
      ...(asString(record.model_service) ? { model_service: asString(record.model_service) } : {}),
      brand: asString(record.brand),
      on_brand: asString(record.on_brand),
      theme_color: { light: asString(theme.light), dark: asString(theme.dark) },
      description: { zh: asString(description.zh), en: asString(description.en) },
      body: asString(record.body),
    }),
  );
}

/** Light surface uses logos.light_surface. Dark surface uses logos.dark_surface. */
export function tenantLogoUrl(info: TenantInfo, surface: 'light' | 'dark'): string {
  const file = surface === 'light' ? info.logos.light_surface : info.logos.dark_surface;
  return `/tenant/${info.slug}/${file}`;
}

/** Transparent window wordmark. Light surface is colored ink; dark surface is light ink. */
export function tenantMarkUrl(slug: string, surface: 'light' | 'dark'): string {
  return `/tenant/${slug}/mark-${surface}.png`;
}
