# Moldavite Clipper

Save the page you are reading as a Markdown note in Moldavite. Text only: links
survive, images and styling do not.

## Install

**Chrome, Edge, Brave**

There is no Chrome Web Store item yet. Build it (see [Development](#development)),
open `chrome://extensions`, turn on **Developer mode**, click **Load unpacked**
and choose `dist/chrome`.

**Firefox**

There is no signed add-on yet, and Firefox installs only signed ones. To try
it, build it and load `dist/firefox/manifest.json` from
`about:debugging#/runtime/this-firefox` as a temporary add-on; Firefox removes
it on restart.

Then open Moldavite → Settings → Plugins and press **Connect browser**. Nothing
can reach your notes until you do.

## Permissions

Three, and no host permissions at all, so the extension has no standing access
to any site:

- `activeTab` — read the page in the tab you are on, and only after you have
  opened the popup on that tab.
- `scripting` — run the small reader script that hands the popup that page's
  HTML.
- `nativeMessaging` — talk to the Moldavite binary.

## How it works

The extension has no filesystem access and makes no network requests — its
`connect-src 'none'` content security policy is what enforces the second. It
talks to the Moldavite binary over the browser's native-messaging channel, which
the browser starts on demand — Moldavite does not have to be running. The bridge
answers exactly two requests:

- `forges` — the names of your Forges, so the dropdown can offer them
- `clip` — write this Markdown to `notes/Clippings/` in the named Forge

Four things leave the browser when you press **Clip this page**, and nothing at
any other time: the page title, the page URL, the Markdown converted from it,
and the Forge you picked.

It cannot read a note. Which extension may connect is pinned by ID in the host
manifest that **Connect browser** writes.

## Limitations

- Whole pages only. A selection is not clipped on its own.
- Text only: images, video, embeds, forms and styling are dropped, and a link
  that is not `http(s)` is unwrapped to plain text.
- Pages no extension may read — `chrome://`, `about:`, the Chrome Web Store, the
  built-in PDF viewer — cannot be clipped.
- Above 5 MB of Markdown the clip is refused rather than truncated.

## Publishing

`npm run build` produces three directories. They differ only in manifest keys.

- `dist/chrome` keeps `key`, which pins the extension id the desktop app allows.
  This is the one to load unpacked.
- `dist/chrome-store` drops `key`, because the Web Store re-signs with its own
  and Chrome refuses a package whose key implies a different id. This is the one
  to upload.
- `dist/firefox` drops `key` and keeps the Gecko id.

A store item gets an id that will not match the unpacked one, and the app only
opens the bridge to ids it knows (`CHROME_EXTENSION_IDS` in
`src-tauri/src/commands/browser_bridge.rs`). That list holds only the unpacked
id today; a store id has to be added there and shipped in an app release.

## Development

```bash
npm install
npm test          # conversion and popup, in jsdom — no browser needed
npm run build     # dist/chrome, dist/chrome-store and dist/firefox
```

`key.pem` is gitignored and kept outside the repository with the other signing
keys; `manifest.json` carries its public half. Chrome derives the extension ID
from that, the host manifest pins the ID, and generating a new key unpairs every
existing install. A Rust test (`browser_bridge.rs`) fails if the manifest key,
the pinned ID, the Gecko id or the host name drift apart.
