# Changelog

All notable changes to the shipped Kiflo desktop app are documented in this file.

The format is based on Keep a Changelog and adapted to this repository's tag-driven release workflow.

## [0.3.4] - 2026-03-17

### Changed
- update icon
- rebrand from Tator to Kiflo
- update coloring of hud
- better side bar

## [0.3.1] - 2026-03-16

### macOS Install Note

This build is not notarized. After dragging Kiflo to `/Applications`, run the following command in Terminal before opening the app:

```
xattr -dr com.apple.quarantine /Applications/Kiflo.app
```

### Changed
- Fix formatting of toggle subtext
- Improve list rendering with auto mode
- Add navigation sidebar with section-based layout
- Add theme selector with system, light, and dark options

## [0.3.0] - 2026-03-13

### Changed
- Replace the bundled Python/MLX transcription helper with a native `whisper.cpp` backend
- Bundle the `whisper.cpp` model directly with the desktop app
- Add Windows GitHub Actions desktop bundles alongside macOS releases
- Improve cross-platform permissions and paste behavior for desktop builds
- Add warm speech model on app launch setting
- Sync the menu bar settings toggles with the main settings panel

## [0.1.8] - 2026-03-13

### Changed
- Remove the unused `apps/api` workspace
- Remove the "stored locally in SQLite" helper copy
- Remove desktop hero section
- Add accessibility access buttons
- Refresh permissions on focus
- Add permissions submenu to menu bar
- Collapse permissions panel

## [0.1.7] - 2026-03-12

### Changed
- Add settings to menu bar icon dropdown

## [0.1.6] - 2026-03-12

### Changed
- Fix media pause behavior during recording
- Keep HUD above fullscreen apps

## [0.1.5] - 2026-03-12

### Changed
- Clarify dictation mode behavior

## [0.1.4] - 2026-03-12

### Changed
- Add release changelog process
- Add transient HUD across spaces
- Refine transient recording HUD behavior
- Add HUD visibility setting

## [0.1.3] - 2026-03-11

### Changed
- Add hide dock icon setting

## [0.1.2] - 2026-03-11

### Changed
- Add missing runtime dependencies

## [0.1.1] - 2026-03-11

### Changed
- Bundle `mlx_whisper` runtime with app
- Reference bundled resources

## [0.1.0] - 2026-03-11

### Changed
- Bootstrap Tauri dictation MVP
- Add verbose transcription logging
- Add background audio pause option
- Fix tray reopen main window
- Add recording cue controls
- Fix dictation accessibility and mic
- Add new app and tray icons
- Bundle MLX model in app
- Update tray icon image
- Add GitHub release workflow
