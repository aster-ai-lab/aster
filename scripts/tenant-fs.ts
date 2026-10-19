/**
 * Filesystem half of the launch-tenant check.
 * The repo root is this file's parent, not process.cwd().
 * Client bundles must not import this module.
 */
import { createHash, randomBytes } from 'node:crypto';
import { lstatSync, readFileSync, readdirSync, realpathSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseTenantInfo, serializeTenantInfo, type TenantInfo } from '../apps/web/src/prefs/tenantInfo.ts';

const SLUG_RE = /^[a-z][a-z0-9-]{0,31}$/;
const MAX_LOGO_BYTES = 524288;

const here = path.dirname(fileURLToPath(import.meta.url));
export const repoRoot = realpathSync(path.resolve(here, '..'));

export type LoadedTenant = {
  info: TenantInfo;
  /** Lowercase sha256 of the INFO.md bytes. */
  digest: string;
  infoPath: string;
};

function inside(parent: string, child: string): boolean {
  const rel = path.relative(parent, child);
  return rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel));
}

function assertRegularFile(filePath: string, label: string) {
  let st;
  try {
    st = lstatSync(filePath);
  } catch {
    throw new Error(`tenant file missing: ${label}: ${filePath}`);
  }
  if (st.isSymbolicLink()) throw new Error(`tenant path is a symlink: ${label}: ${filePath}`);
  if (!st.isFile()) throw new Error(`tenant path is not a file: ${label}: ${filePath}`);
}

const PNG_MAGIC = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

function assertPng(bytes: Buffer) {
  if (bytes.length < PNG_MAGIC.length || !bytes.subarray(0, PNG_MAGIC.length).equals(PNG_MAGIC)) {
    throw new Error('logo is not a png');
  }
  if (bytes.length > MAX_LOGO_BYTES) throw new Error(`logo is larger than ${MAX_LOGO_BYTES}`);
}

export function decodePngDataUrl(value: string): Buffer {
  const prefix = 'data:image/png;base64,';
  if (!value.startsWith(prefix)) throw new Error('logo must be a png data url');
  const bytes = Buffer.from(value.slice(prefix.length), 'base64');
  assertPng(bytes);
  return bytes;
}

type TenantPaths = { tenantDir: string; dirReal: string; infoPath: string };

function resolveTenantDir(slug: string): TenantPaths {
  if (!SLUG_RE.test(slug)) throw new Error(`invalid tenant slug: ${slug}`);
  const tenantDir = path.join(repoRoot, 'apps/web/public/tenant', slug);
  let dirStat;
  try {
    dirStat = lstatSync(tenantDir);
  } catch {
    throw new Error(`tenant directory missing: ${slug}: ${tenantDir}`);
  }
  if (dirStat.isSymbolicLink()) {
    throw new Error(`tenant directory is a symlink: ${slug}: ${tenantDir}`);
  }
  if (!dirStat.isDirectory()) {
    throw new Error(`tenant path is not a directory: ${slug}: ${tenantDir}`);
  }
  const parentReal = realpathSync(path.dirname(tenantDir));
  const dirReal = realpathSync(tenantDir);
  const expectedDir = path.join(parentReal, slug);
  if (dirReal !== expectedDir || !inside(repoRoot, dirReal)) {
    throw new Error(`tenant directory escapes the repo: ${slug}: ${tenantDir}`);
  }
  return { tenantDir, dirReal, infoPath: path.join(tenantDir, 'INFO.md') };
}

export function listTenantSlugs(): string[] {
  const parent = path.join(repoRoot, 'apps/web/public/tenant');
  let names: string[] = [];
  try {
    names = readdirSync(parent);
  } catch {
    return [];
  }
  const slugs: string[] = [];
  for (const name of names) {
    if (!SLUG_RE.test(name)) continue;
    try {
      resolveTenantDir(name);
    } catch {
      continue;
    }
    slugs.push(name);
  }
  slugs.sort();
  return slugs;
}

export type TenantRecord = {
  slug: string;
  info: TenantInfo | null;
  error: string | null;
};

export function readTenantRecord(slug: string): TenantRecord {
  try {
    return { slug, info: loadTenant(slug).info, error: null };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { slug, info: null, error: message };
  }
}

export type TenantLogoBytes = { light?: Buffer; dark?: Buffer };

function writeAtomic(dirReal: string, filename: string, bytes: Buffer) {
  if (filename.includes('/') || filename.includes('..') || filename.startsWith('.')) {
    throw new Error(`tenant file name is not allowed: ${filename}`);
  }
  const dest = path.join(dirReal, filename);
  if (!inside(dirReal, dest)) throw new Error(`tenant file escapes the directory: ${filename}`);
  const tmp = path.join(dirReal, `.${filename}.${randomBytes(6).toString('hex')}.tmp`);
  writeFileSync(tmp, bytes, { flag: 'wx' });
  try {
    renameSync(tmp, dest);
  } catch (error) {
    try {
      unlinkSync(tmp);
    } catch {
      /* the temp file is removed on the next attempt */
    }
    throw error;
  }
}

function restoreFile(dirReal: string, filename: string, previous: Buffer | null) {
  if (previous) {
    writeAtomic(dirReal, filename, previous);
    return;
  }
  try {
    unlinkSync(path.join(dirReal, filename));
  } catch {
    /* nothing to remove */
  }
}

function readIfFile(filePath: string): Buffer | null {
  try {
    const st = lstatSync(filePath);
    if (!st.isFile() || st.isSymbolicLink()) return null;
    return readFileSync(filePath);
  } catch {
    return null;
  }
}

/** Writes INFO.md and optional PNG logos for an existing tenant directory. Restores both on failure. */
export function saveTenant(slug: string, info: TenantInfo, logos: TenantLogoBytes = {}): LoadedTenant {
  if (info.slug !== slug) {
    throw new Error(`tenant ${slug}: INFO.md field slug: must equal ${slug}`);
  }
  const { dirReal, infoPath } = resolveTenantDir(slug);
  assertRegularFile(infoPath, 'INFO.md');
  const text = serializeTenantInfo(info);
  const lightName = info.logos.light_surface;
  const darkName = info.logos.dark_surface;
  if (logos.light) assertPng(logos.light);
  if (logos.dark) assertPng(logos.dark);

  const previousInfo = readFileSync(infoPath);
  const previousLight = logos.light ? readIfFile(path.join(dirReal, lightName)) : null;
  const previousDark = logos.dark ? readIfFile(path.join(dirReal, darkName)) : null;
  if (logos.light && !previousLight) {
    throw new Error(`tenant ${slug}: ${lightName}: file missing`);
  }
  if (logos.dark && !previousDark) {
    throw new Error(`tenant ${slug}: ${darkName}: file missing`);
  }
  try {
    if (logos.light) writeAtomic(dirReal, lightName, logos.light);
    if (logos.dark) writeAtomic(dirReal, darkName, logos.dark);
    writeAtomic(dirReal, 'INFO.md', Buffer.from(text, 'utf8'));
    return loadTenant(slug);
  } catch (error) {
    if (logos.light) restoreFile(dirReal, lightName, previousLight);
    if (logos.dark) restoreFile(dirReal, darkName, previousDark);
    restoreFile(dirReal, 'INFO.md', previousInfo);
    throw error;
  }
}

export function loadTenant(slug: string): LoadedTenant {
  const { tenantDir, dirReal, infoPath } = resolveTenantDir(slug);

  assertRegularFile(infoPath, 'INFO.md');
  const infoReal = realpathSync(infoPath);
  if (infoReal !== path.join(dirReal, 'INFO.md') || !inside(dirReal, infoReal)) {
    throw new Error(`tenant INFO.md escapes the directory: ${slug}: ${infoPath}`);
  }
  const bytes = readFileSync(infoPath);
  let info: TenantInfo;
  try {
    info = parseTenantInfo(bytes.toString('utf8'));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`tenant ${slug}: ${infoPath}: ${message}`);
  }
  if (info.slug !== slug) {
    throw new Error(`tenant ${slug}: ${infoPath}: INFO.md field slug: must equal ${slug}`);
  }

  for (const [field, filename] of [
    ['logos.light_surface', info.logos.light_surface],
    ['logos.dark_surface', info.logos.dark_surface],
  ] as const) {
    if (filename.includes('/') || filename.includes('..')) {
      throw new Error(`tenant ${slug}: ${infoPath}: INFO.md field ${field}: must be a single filename`);
    }
    const logoPath = path.join(tenantDir, filename);
    let st;
    try {
      st = lstatSync(logoPath);
    } catch {
      throw new Error(`tenant ${slug}: ${logoPath}: INFO.md field ${field}: file missing`);
    }
    if (st.isSymbolicLink()) {
      throw new Error(`tenant ${slug}: ${logoPath}: INFO.md field ${field}: symlink`);
    }
    if (!st.isFile()) {
      throw new Error(`tenant ${slug}: ${logoPath}: INFO.md field ${field}: not a file`);
    }
    if (st.size > MAX_LOGO_BYTES) {
      throw new Error(`tenant ${slug}: ${logoPath}: INFO.md field ${field}: larger than ${MAX_LOGO_BYTES}`);
    }
    const logoReal = realpathSync(logoPath);
    if (logoReal !== path.join(dirReal, filename) || !inside(dirReal, logoReal)) {
      throw new Error(`tenant ${slug}: ${logoPath}: INFO.md field ${field}: escapes the directory`);
    }
  }

  const digest = createHash('sha256').update(bytes).digest('hex');
  return { info, digest, infoPath };
}
