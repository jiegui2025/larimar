# Archived Tauri iOS app

Upstream shipped this Tauri app on iPhone and iPad. Larimar does not build it:
its planned iOS app is SwiftUI on the shared Rust core through UniFFI ([#14],
ADR 0002). The owner chose to archive the Tauri app rather than delete it, so it
can serve as a reference or an earlier start (decision D6 in [#1]). [#15] took it
off `main`.

## Where it lives

| Ref | Name | Commit |
| --- | --- | --- |
| Branch | `archive/tauri-ios` | `6996b61` (upstream 2.11.0, the last `main` commit before [#15]) |
| Annotated tag | `archive-tauri-ios-2.11.0` | `6996b61` |

The "Protect archives" ruleset blocks deleting, updating or force-pushing both.
Fetch the tag with `git fetch origin tag archive-tauri-ios-2.11.0`.

## What the archive holds

| Path at the tag | Files | Contents |
| --- | --- | --- |
| `src-tauri/gen/apple/` | 33 | Xcode project: XcodeGen `project.yml`, `project.pbxproj`, Podfile, app `Info.plist` and entitlements, icon catalog, `ExportOptions.plist` |
| `src-tauri/ios/` | 4 | App and widget privacy manifests; the WidgetKit home-screen widget (today's date, opens the daily note) |
| `src-tauri/plugins/` | 40 | Four iOS-only Tauri plugins, Rust and Swift (table below) |
| `src-tauri/tauri.ios.conf.json` | 1 | iOS overrides (no Markdown file associations) |
| `src-tauri/capabilities/mobile.json` | 1 | Capability for iOS and Android windows |
| `src-tauri/icons/ios/` | 18 | iPhone, iPad and App Store icon PNGs |
| `branding/app-store/`, `branding/app-icons/`, `branding/opaque-png.swift` | 15 | App Store screenshots and sample notes, the 1024 px icon master, the helper that strips alpha from App Store icons |
| `docs/MOBILE.md`, `docs/MOBILE_QA.md` | 2 | Build guide, feature status and remaining work; device QA checklist |
| `docs/IOS_APP_STORE.md`, `docs/IOS_STORE_LISTING.json` | 2 | App Store Connect handoff and listing copy |

| Plugin | Purpose |
| --- | --- |
| `tauri-plugin-icloud` | iCloud Drive bridge for iOS: resolves the container, reports download, upload and conflict state, streams `NSMetadataQuery` changes |
| `tauri-plugin-calendar` | Links the `src-swift` EventKit bridge on iOS and runs Google consent in `ASWebAuthenticationSession` |
| `tauri-plugin-document-export` | Export or share a finished file through the document picker or share sheet |
| `tauri-plugin-mobile-ui` | Native appearance and Dynamic Type for the webview |

The tag also holds the iOS code paths in shared Rust files, listed under
[Restore](#restore).

## State at archive time

- **Released upstream:** 2.7.2 and 2.9.1 were live on the App Store; 2.10.1 was
  exported for upload. Minimum iOS and iPadOS 17.0.
- **Upstream identity throughout:** upstream's Apple team ID (also in
  `project.pbxproj`), upstream's iCloud container, upstream's app and widget
  bundle identifiers, its App Store record, and its public Google OAuth client
  for iOS (the default in `calendar/oauth.rs`).
- **Gaps:** no App Group (the widget shares no data with the app), no Share
  Extension, no Markdown document types on iOS.
- **Build needs:** a Mac with full Xcode, the iOS Rust targets, CocoaPods and
  the rustup `llvm-tools` component (`docs/MOBILE.md` at the tag).

## What stayed on `main`

The macOS synced Forge uses the same Foundation bridge as the iOS app, so [#15]
moved it out of the iCloud plugin instead of archiving it:

| On `main` | At the tag |
| --- | --- |
| `src-tauri/src-swift-cloud/` (Swift package) | `src-tauri/plugins/tauri-plugin-icloud/ios/Core/` |
| `src-tauri/src/file_coordination.rs` | `src-tauri/plugins/tauri-plugin-icloud/src/coordination.rs` |
| `src-tauri/src/cloud_forge/models.rs` | `src-tauri/plugins/tauri-plugin-icloud/src/models.rs` |

The `test-icloud` CI job stays and now tests `src-tauri/src-swift-cloud`.

Also unchanged: the `src-swift` EventKit bridge (macOS), the `cfg(mobile)` and
`cfg(desktop)` gates in `src-tauri/src`, and the React app's phone layout
(`src/lib/platform.ts`, `src/mobile.css` and the components that use them).

## Restore

1. Restore the removed files byte for byte:

   ```sh
   git checkout archive-tauri-ios-2.11.0 -- \
     src-tauri/gen/apple src-tauri/ios src-tauri/plugins \
     src-tauri/tauri.ios.conf.json src-tauri/capabilities/mobile.json \
     src-tauri/icons/ios branding/app-store branding/app-icons \
     branding/opaque-png.swift docs/MOBILE.md docs/MOBILE_QA.md \
     docs/IOS_APP_STORE.md docs/IOS_STORE_LISTING.json
   ```

2. Point the restored iCloud plugin at `main`'s bridge, or it compiles a second
   copy. Delete its `ios/Core/`, `src/coordination.rs` and `src/models.rs`;
   in its `src/lib.rs` put `#[path = "../../../src/file_coordination.rs"]` above
   `pub mod coordination;` and `#[path = "../../../src/cloud_forge/models.rs"]`
   above `pub mod models;`; in its `ios/Package.swift` change the bridge
   package's `path: "Core"` to `path: "../../../src-swift-cloud"`. The commit
   found by `git log --grep='move the macOS iCloud bridge out of the iOS plugin'`
   makes exactly this change.
3. Put back the iOS code paths. These files lost them; compare each with
   `git diff archive-tauri-ios-2.11.0 -- <file>` (`main` has other changes too):

   | File | Removed |
   | --- | --- |
   | `src-tauri/Cargo.toml`, `Cargo.lock` | The iOS dependency section (system `rusqlite` and the four plugins), `staticlib` in `crate-type`, iOS and Android in the target conditions |
   | `src-tauri/build.rs` | iOS and Android in the `semantic_runtime` rule |
   | `src-tauri/tauri.conf.json` | `bundle.iOS` (team, minimum system version) |
   | `src-tauri/src/lib.rs` | Plugin registration, the `export_mobile_document` and `share_mobile_note` commands, `target_os = "ios"` beside `"macos"` |
   | `src-tauri/src/cloud_forge.rs` | The `tauri_plugin_icloud` connection and `std::os::ios` metadata |
   | `src-tauri/src/cloud_forge/models.rs` | The `Container` type; `target_os = "ios"` in two cfgs |
   | `src-tauri/src/file_coordination.rs` | `target_os = "ios"` on `read` and `write` (now test-only) |
   | `src-tauri/src/note_file_access.rs` | The plugin's coordination as `native` on iOS |
   | `src-tauri/src/calendar/oauth.rs` | The iOS OAuth client, its redirect and its consent flow, and their tests |
   | `src-tauri/src/calendar/mod.rs` | `target_os = "ios"` beside `"macos"` |
   | `src-tauri/src/commands/export_import.rs` | `MobileExport` staging, the two document commands, and their tests |
   | `src-tauri/src/commands/forges.rs`, `commands/locking.rs`, `commands/notes.rs` | `target_os = "ios"` beside `"macos"`; the fixed Forges folder |
   | `src-tauri/src/paths.rs` | The container-relative Forges root |
   | `src-tauri/src/persist.rs` | `std::os::ios` file times |
   | `scripts/bump-version.mjs` | The Xcode project and the two iOS `Info.plist` files as version manifests |
   | `branding/generate.mjs` | iOS icon generation |
   | `.gitignore` | Ignores for Xcode build products |

   Reverting the commit found by
   `git log --grep='remove the archived Tauri iOS app from main'` does steps 1
   to 3 at once, with conflicts wherever `main` has changed those lines since.
   That route restores the plugins already pointed at `main`'s bridge, and
   takes this note and its README link back out.

## What a revival needs

| Need | Why |
| --- | --- |
| Larimar's Apple developer team | Every signing setting names upstream's Apple team ID |
| Larimar bundle identifiers for the app and widget | The archive uses upstream's |
| Larimar's own iCloud container | The archive uses upstream's iCloud container; the bridge in `src-swift-cloud` names it too ([#4]) |
| An App Group | For the widget, and a Share Extension, to read the app's data ([#14]) |
| Larimar's own Google OAuth client for iOS | The archived default is upstream's |
| Rebrand and a new App Store record | Names, icons, screenshots and listing copy are upstream's ([#16]) |
| A decision against SwiftUI + UniFFI | ADR 0002 plans a native app instead; decide when [#14] starts |

[#1]: https://github.com/jiegui2025/larimar/issues/1
[#4]: https://github.com/jiegui2025/larimar/issues/4
[#14]: https://github.com/jiegui2025/larimar/issues/14
[#15]: https://github.com/jiegui2025/larimar/issues/15
[#16]: https://github.com/jiegui2025/larimar/issues/16
