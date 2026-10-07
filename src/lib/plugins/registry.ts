/**
 * Validation and fixed-origin URL construction for the public plugin registry.
 * Registry JSON is untrusted display metadata: file downloads are derived only
 * from a validated id and the pinned repository below.
 */

import { isNewerVersion } from '@/lib/changelog';
import { isValidAllowedHost } from './manifest';
import { SUPPORTED_PLUGIN_API_VERSIONS, type PluginInfo } from './types';

/**
 * The pinned GitHub repository (`owner/name`) whose `main` branch holds
 * `registry.json` and `plugins/<id>/`. Larimar has no plugin registry yet, so
 * this is null: the directory lists nothing and browsing it sends no request.
 * Setting it also needs `https://raw.githubusercontent.com` back in the
 * webview CSP's `connect-src` (tauri.conf.json), which the page fetches from.
 */
export const COMMUNITY_REGISTRY_REPO: string | null = null;

export function communityRegistryUrl(repo: string): string {
  return `https://raw.githubusercontent.com/${repo}/main/registry.json`;
}

const ID_RE = /^[a-z0-9][a-z0-9-]*$/;
const SHA256_RE = /^[a-f0-9]{64}$/;
const MAX_REGISTRY_PLUGINS = 500;

export interface CommunityPlugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  apiVersion: number;
  minAppVersion?: string;
  permissions: string[];
  allowedHosts: string[];
  files: {
    'manifest.json': { sha256: string };
    'plugin.js': { sha256: string };
  };
}

export interface ParsedCommunityRegistry {
  plugins: CommunityPlugin[];
  rejectedEntries: number;
}

export type CommunityInstallState = 'not-installed' | 'installed' | 'update-available';

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function boundedString(value: unknown, max: number): string | null {
  return typeof value === 'string' && value.trim().length > 0 && value.length <= max ? value : null;
}

function stringArray(value: unknown, maxItems: number, maxLength: number): string[] | null {
  if (!Array.isArray(value) || value.length > maxItems) return null;
  const strings = value.filter(
    (item): item is string =>
      typeof item === 'string' && item.length > 0 && item.length <= maxLength
  );
  if (strings.length !== value.length || new Set(strings).size !== strings.length) return null;
  return strings;
}

function registryHash(files: Record<string, unknown>, name: 'manifest.json' | 'plugin.js') {
  const file = record(files[name]);
  return file && typeof file.sha256 === 'string' && SHA256_RE.test(file.sha256)
    ? file.sha256
    : null;
}

function parseEntry(value: unknown): CommunityPlugin | null {
  const entry = record(value);
  if (!entry) return null;

  const id = boundedString(entry.id, 64);
  const name = boundedString(entry.name, 160);
  const version = boundedString(entry.version, 64);
  const description = boundedString(entry.description, 1_000);
  const author = boundedString(entry.author, 160);
  const apiVersion = entry.apiVersion;
  const minAppVersion = entry.minAppVersion;
  const permissions = stringArray(entry.permissions, 50, 128);
  const allowedHosts = stringArray(entry.allowedHosts, 50, 253);
  const files = record(entry.files);
  const path = boundedString(entry.path, 128);

  if (
    !id ||
    !ID_RE.test(id) ||
    !name ||
    !version ||
    !description ||
    !author ||
    typeof apiVersion !== 'number' ||
    !Number.isInteger(apiVersion) ||
    apiVersion < 1 ||
    apiVersion > 99 ||
    (minAppVersion !== undefined && !boundedString(minAppVersion, 64)) ||
    !permissions ||
    !allowedHosts ||
    allowedHosts.some((host) => !isValidAllowedHost(host)) ||
    !files ||
    !path ||
    path.replace(/\/$/, '') !== `plugins/${id}`
  ) {
    return null;
  }

  const manifestHash = registryHash(files, 'manifest.json');
  const pluginHash = registryHash(files, 'plugin.js');
  if (!manifestHash || !pluginHash) return null;

  return {
    id,
    name,
    version,
    description,
    author,
    apiVersion,
    ...(typeof minAppVersion === 'string' ? { minAppVersion } : {}),
    permissions,
    allowedHosts,
    files: {
      'manifest.json': { sha256: manifestHash },
      'plugin.js': { sha256: pluginHash },
    },
  };
}

/** Parse the v1 envelope and keep valid entries without letting one bad row hide the rest. */
export function parseCommunityRegistry(value: unknown): ParsedCommunityRegistry {
  const registry = record(value);
  if (!registry || registry.registryVersion !== 1 || !Array.isArray(registry.plugins)) {
    throw new Error('The community registry response was not in the expected format.');
  }
  if (registry.plugins.length > MAX_REGISTRY_PLUGINS) {
    throw new Error('The community registry contains too many entries.');
  }

  const plugins: CommunityPlugin[] = [];
  const seen = new Set<string>();
  let rejectedEntries = 0;
  for (const value of registry.plugins) {
    const plugin = parseEntry(value);
    if (!plugin || seen.has(plugin.id)) {
      rejectedEntries += 1;
      continue;
    }
    seen.add(plugin.id);
    plugins.push(plugin);
  }
  return { plugins, rejectedEntries };
}

/** Never accepts a registry path or URL: only a validated registry id reaches this helper. */
export function communityPluginFileUrl(
  repo: string,
  plugin: CommunityPlugin,
  file: 'manifest.json' | 'plugin.js'
): string {
  return `https://raw.githubusercontent.com/${repo}/main/plugins/${plugin.id}/${file}`;
}

export function communityPluginSourceUrl(repo: string, plugin: CommunityPlugin): string {
  return `https://github.com/${repo}/tree/main/plugins/${plugin.id}`;
}

export function communityReportUrl(repo: string, plugin: CommunityPlugin): string {
  const title = encodeURIComponent(`Report: ${plugin.id}`);
  return `https://github.com/${repo}/issues/new?template=report-plugin.yml&title=${title}`;
}

/** Why this app can't run a listed plugin, or `null` when it can. */
export function communityIncompatibility(
  plugin: CommunityPlugin,
  appVersion: string
): string | null {
  if (plugin.minAppVersion && isNewerVersion(plugin.minAppVersion, appVersion)) {
    return `Needs Moldavite ${plugin.minAppVersion} or later`;
  }
  if (!(SUPPORTED_PLUGIN_API_VERSIONS as readonly number[]).includes(plugin.apiVersion)) {
    return 'Needs a newer version of Moldavite';
  }
  return null;
}

export function communityInstallState(
  plugin: CommunityPlugin,
  installedPlugins: readonly PluginInfo[]
): CommunityInstallState {
  const installed = installedPlugins.find((info) => info.manifest.id === plugin.id);
  if (!installed) return 'not-installed';
  return installed.status === 'ok' && installed.manifest.version === plugin.version
    ? 'installed'
    : 'update-available';
}
