/**
 * Validate one launch tenant. Success stdout is 64 lowercase hex digits and a LF.
 * Usage errors exit 2. Invalid slug, missing files, and bad INFO.md exit 1.
 */
import { loadTenant } from './tenant-fs.ts';

const args = process.argv.slice(2);
if (args.length !== 1 || args[0] === '') {
  console.error('usage: bun scripts/check-tenant.ts <slug>');
  process.exit(2);
}

const slug = args[0] ?? '';
try {
  const loaded = loadTenant(slug);
  process.stdout.write(`${loaded.digest}\n`);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(message);
  process.exit(1);
}
