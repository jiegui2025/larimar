#!/usr/bin/env node
/**
 * Fails when a file over 1 MiB enters git, unless `.github/large-files-allowlist`
 * names it with a reason. Git keeps every blob forever, so a binary pushed by
 * mistake can only be removed by rewriting history; builds belong in releases
 * and Actions artifacts instead.
 *
 *   node scripts/check-large-files.mjs <base> <head>   files added or changed since base
 *   node scripts/check-large-files.mjs --staged        the index (pre-commit hook)
 *   node scripts/check-large-files.mjs --all           every tracked file
 *
 * A base of all zeros (a push that created the branch) checks every file.
 * Renames and type changes count as additions: `--no-renames --diff-filter=d`
 * keeps everything but deletions, so a moved or grown file can't slip through.
 */
import { existsSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const LIMIT = 1024 * 1024;
const ALLOWLIST = '.github/large-files-allowlist';

function git(args) {
  return execFileSync('git', args, { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

function nulList(output) {
  return output.split('\0').filter(Boolean);
}

/** `path  # reason` per line; a line without a reason is an error, not an entry. */
function parseAllowlist(text) {
  const paths = new Set();
  const errors = [];
  text.split('\n').forEach((raw, i) => {
    const line = raw.trim();
    if (!line || line.startsWith('#')) return;
    const match = /^(\S+)\s+#\s*(\S.*)$/.exec(line);
    if (match) paths.add(match[1]);
    else errors.push(`${ALLOWLIST}:${i + 1}: expected "path  # reason", got "${line}"`);
  });
  return { paths, errors };
}

/** [path, size] for each file the mode selects. */
function candidates(argv) {
  if (argv[0] === '--staged') {
    const files = nulList(
      git(['diff', '--cached', '--name-only', '--no-renames', '--diff-filter=d', '-z'])
    );
    return files.map((f) => [f, Number(git(['cat-file', '-s', `:${f}`]))]);
  }
  const all = argv[0] === '--all' || /^0+$/.test(argv[0] ?? '');
  const head = all ? (argv[0] === '--all' ? 'HEAD' : argv[1]) : argv[1];
  // An empty base would diff against the checkout and pass vacuously.
  if (!head || (!all && !argv[0])) {
    console.error('usage: check-large-files.mjs <base> <head> | --staged | --all');
    process.exit(2);
  }
  const files = all
    ? nulList(git(['ls-tree', '-r', '-z', '--name-only', head]))
    : nulList(
        git([
          'diff',
          '--name-only',
          '--no-renames',
          '--diff-filter=d',
          '-z',
          `${argv[0]}...${head}`,
        ])
      );
  // `ls-tree -l` reports every size in one call instead of a process per file.
  const sizes = new Map(
    git(['ls-tree', '-r', '-l', '-z', head])
      .split('\0')
      .filter(Boolean)
      .map((entry) => {
        const tab = entry.indexOf('\t'); // paths may contain tabs; the first one ends the metadata
        return [entry.slice(tab + 1), Number(entry.slice(0, tab).trim().split(/\s+/)[3])];
      })
  );
  const unsized = files.filter((f) => !sizes.has(f));
  if (unsized.length) {
    // Never drop a file silently: a parsing mismatch must fail, not pass.
    console.error(`no size found for: ${unsized.join(', ')}`);
    process.exit(2);
  }
  return files.map((f) => [f, sizes.get(f)]);
}

function main(argv) {
  const { paths: allowed, errors } = existsSync(ALLOWLIST)
    ? parseAllowlist(readFileSync(ALLOWLIST, 'utf8'))
    : { paths: new Set(), errors: [] };
  const tooLarge = candidates(argv).filter(([f, size]) => size > LIMIT && !allowed.has(f));

  for (const error of errors) console.error(error);
  for (const [f, size] of tooLarge) {
    console.error(
      `${f}: ${(size / LIMIT).toFixed(2)} MiB is over the 1 MiB limit. ` +
        `Publish it as a release asset or artifact, or add it to ${ALLOWLIST} with a reason.`
    );
  }
  if (errors.length || tooLarge.length) process.exit(1);
}

main(process.argv.slice(2));
