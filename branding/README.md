# Moldavite brand kit

Use the PNG files for social uploads and SVG files for layouts, print, or resizing.
The SVG artwork is outlined: no font installation is needed. These are the same
monogram and wordmark used in the app, without a new logo or altered lettering.

| Folder | What to use |
| --- | --- |
| `logos/` | Transparent monogram (1024px) and wordmark (2400px wide), in ink or cream. Use ink on light backgrounds and cream on dark backgrounds. |
| `social/` | Light and dark profile pictures (1080×1080), banners (1500×500), and landscape posts/link cards (1200×630). |

The profile pictures leave room for circular cropping. Social platforms crop
headers differently on phones and computers; check the platform's preview before
publishing. The included dimensions are reusable canvases, not a promise that
one file matches every platform's current requirements.

Colors: cream `#F9F6ED`, ink `#0E0D0A`, dark ground `#14120C`.
Keep the artwork proportional, leave clear space around it, and avoid shadows,
extra outlines or recoloring. The app icon intentionally uses the dark ground.

## Regenerate

From the repository root, with Node 22:

```sh
npm install --prefix /tmp/moldavite-brand-render --no-audit --no-fund @resvg/resvg-js@2.6.2
node branding/generate.mjs /tmp/moldavite-brand-render
```

This uses `public/monogram.svg` and `public/wordmark.svg`, and updates only the
brand kit. The renderer is installed outside the app's dependencies.
