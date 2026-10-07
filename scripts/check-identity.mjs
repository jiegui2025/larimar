#!/usr/bin/env node
/**
 * Fails when a tracked file carries one of upstream's runtime identifiers:
 * its bundle id and iCloud container, clipper host and ids, deep-link scheme,
 * Apple team, build-time variable names, on-disk format and file names, or
 * storage keys. Any of them in a build would share state with an installed
 * copy of the upstream app or open its data (ADR 0008).
 *
 *   node scripts/check-identity.mjs
 *
 * EXCLUDED holds history that may name them: the changelog, the release guide
 * until #17 rewrites it, ADRs and the archive. Upstream's product name itself
 * is gated separately by #16.
 */
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// Basic regular expressions, case-sensitive: `MOLDAVITE_[A-Z]` catches the
// backup header and the build-time variable names without matching prose.
const PATTERNS = [
  'app\\.moldavite',
  'com\\.moldavite',
  'moldavite://',
  'J6Z5WJKHZB',
  'moldavite-note',
  'MOLDAVITE_[A-Z]',
  'iCloud\\.app\\.moldavite',
  'clipper@moldavite',
  'moldavite-backup',
  'moldavite-import',
  'moldaviteDrop',
  '__moldavite',
  // Upstream's unpacked clipper id; its key is no longer in the manifest.
  'dgidmimgcpmanonfbijebppdmfhnhhem',
];

const EXCLUDED = [
  'CHANGELOG.md',
  'docs/RELEASING.md',
  'docs/adr/**',
  'docs/archive/**',
  'scripts/check-identity.mjs',
];

const args = ['grep', '-n', '-I', ...PATTERNS.flatMap((pattern) => ['-e', pattern])];
args.push('--', '.', ...EXCLUDED.map((path) => `:(exclude,glob)${path}`));

try {
  const hits = execFileSync('git', args, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 64 * 1024 * 1024,
  });
  process.stderr.write(hits);
  console.error(
    "Upstream's runtime identifiers are still in the tree (above). Use Larimar's, " +
      'or add a path to EXCLUDED in scripts/check-identity.mjs if it is history.'
  );
  process.exit(1);
} catch (error) {
  // git grep exits 1 when nothing matches; anything else is a failed search.
  if (error.status === 1 && !error.stdout) {
    console.log('No upstream runtime identifiers found.');
  } else {
    console.error(error.stderr || error.message);
    process.exit(2);
  }
}
