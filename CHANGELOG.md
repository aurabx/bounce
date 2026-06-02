# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed

- Quitting the application now gracefully shuts down the DICOM receiver and releases its TCP listener, instead of leaving the process running in the background with the port bound. The window's close button still hides the app to the background as before; only a real exit (Cmd+Q, File → Quit, tray Quit, app.exit) terminates the process. This unblocks restart-then-start cycles that previously failed with "address already in use" (AURA-2289).

## [1.7.0] - 2026-05-30

### Added

- Added a "Start on login" setting (Settings → Startup) that registers Bounce with the operating system's login items on macOS and Windows, so the application launches automatically when the user signs in and the receiver returns after a reboot.
- Added an optional automatic-update mode that restarts Bounce to apply a downloaded update without manual intervention, gated by a configurable restart time window so the service is not interrupted during busy hours. When an automatic restart occurs while the DICOM receiver is running, the receiver is started again on the next launch so the service returns to the same state.
- Added a Transactions view: a new sidebar menu item listing the upload-attempt history across all studies, with server-side pagination and free-text search (Study UID, upload ID, status, error).
- Added automatic upload retry with recovery on restart and a disk-space safety check that prevents new uploads when free space is low.

### Changed

- Replaced the dashboard study count with a status summary and a list of recent transactions.

### Fixed

- Hardened the backend against malformed input and runtime failures: DICOM UIDs are validated before building storage paths, unsupported transfer syntaxes are rejected instead of panicking, malformed config values fall back to defaults, and commands return errors rather than panicking or exiting the process.
- Surfaced storage-directory and IPC event-emit failures through error reporting instead of unwrapping or terminating the process.
- Redacted the upload token from logs.
- Surfaced event-binding failures in the UI and used stable list keys.

### Internal

- Added typed wrappers for Tauri commands and removed `ts-ignore` usages, resolving TypeScript errors across Settings, plugin-store, and studies search.
- Guarded `runningDetail` JSON parsing in the reducer and removed an empty `client-side-button.js`.

## [1.6.1] - 2026-05-30

### Fixed

- Fixed automatic updates failing with a 404 during download. The updater manifest (`latest.json`) referenced artifact filenames that GitHub rewrites on upload (spaces become dots, and the two macOS bundles gain an architecture suffix), so the download URLs did not exist. The release workflow now resolves filenames from the assets actually uploaded to the release.

## [1.6.0] - 2026-05-28

### Added

- Added server-side study search across description, patient name, patient ID, accession number, and Study UID.
- Added bulk Send and bulk Delete for selected studies, plus a Delete-all action, each guarded by a confirmation dialog.
- Added a 25/50/100 page-size selector and a click-to-copy, resizable-column Studies table.

### Changed

- Replaced the Studies card list with a compact table and corrected the pagination totals to use the backend count rather than the current page length.
- Enlarged the default window to 1280x820 and narrowed the sidebar.

### Internal

- Added a test covering the filtered pagination query.

## [1.5.0] - 2026-05-26

### Added

- Added automatic update checking against GitHub releases with silent background downloads, an in-app banner that prompts to restart when an update is ready, and a Settings card to trigger manual checks.

### Internal

- Publish workflow now signs each platform's updater artifact and assembles a `latest.json` manifest from the per-platform signatures so the Tauri updater can resolve the latest release.

## [1.4.1] - 2026-05-24

### Changed

- C-ECHO verification now polls the Aura backend for pending verifications so peer status reflects upstream results rather than only the most recent local attempt.

## [1.4.0] - 2026-05-23

### Added

- Added C-MOVE SCP so workstations can initiate retrieves through Bounce.
- Added an outbound C-STORE SCU and send pipeline for forwarding studies to remote PACS.
- Added a PACS services tab with C-ECHO verification and a local cache of configured peers.
- Added API key obfuscation in the settings UI and a connectivity verification action.

### Changed

- C-STORE send now retries transient failures by reopening the association rather than failing the whole pipeline.

### Documentation

- Documented the DICOM endpoint model and how Bounce communicates with Aurabox.
- Documented throughput limits and the dominant bottlenecks in the send pipeline.

### Build / CI

- Upgraded `actions/checkout` and `actions/setup-node` to v6 in the publish workflow.

### Internal

- Cleared a Clippy `enum_variant_names` lint on `PendingDimseCommand`, removed an orphan doc comment in C-FIND extraction helpers, and dropped a redundant closure in C-STORE SOP collection.
- Cleaned up agent configuration files and removed `.agents` / `.claude` / `.codex` directories from the git index.
- Aligned `tauri-plugin-dialog` (2.7.1) and `tauri-plugin-fs` (2.5.1) in `Cargo.lock` so the Rust crates match the resolved JS plugin versions Tauri checks at build time.

## [1.3.0] - 2026-03-13

### Added

- Implemented C-FIND SCP to proxy inbound DICOM queries to the Aura backend.
- Added DIMSE request and response traffic logging for inbound associations.
- Streamed backend logs into the app UI for real-time visibility.

### Fixed

- Fixed C-FIND response to send the command PDU before the dataset PDU, matching the DICOM standard ordering.
- Fixed DIMSE query handling to wait for complete datasets before processing requests.
- Fixed opening the log file from the app log directory.

### Internal

- Added Automatic agent project configuration and linked Claude skills to local agent skills.
- Added `bounce-release` skill for the release workflow.

## [1.2.1] - 2026-02-21

### Fixed
- Dynamically allocate Next.js dev server port to avoid `EADDRINUSE` errors when the default port is already in use.

## [1.2.0] - 2026-02-18

### Added
- Implemented PACS query support with a new DICOM C-FIND SCU workflow.
- Extended C-FIND querying to support PATIENT-level queries.
- Added automated tests for C-FIND query behavior and coverage for PATIENT-level lookups.
- Added a `Makefile` with common development tasks.

### Changed
- Added a dynamic toggle for remote logging and enabled the C-FIND PACS query command in the app flow.
- Updated developer documentation for C-FIND implementation and project agent guidance (`WARP.md` renamed to `AGENTS.md`).

### Fixed
- Corrected the log file path and aligned documentation with the actual platform log locations.

## [1.1.0] - 2025-11-21

### Added
- Integrated **shadcn/ui** component library for a consistent and modern design system.
- Added new UI components: `Button`, `Card`, `Input`, `Label`, `Badge`, `Alert`.
- Added **Heroicons** to the sidebar navigation.
- Reintroduced **Indigo** as the primary brand color across the application (buttons, active states, focus rings).

### Changed
- **UI Overhaul**:
    - Redesigned **Sidebar** with a dark theme (`slate-900`) and improved typography.
    - Updated **Dashboard** to use card-based layout for status and stats.
    - Refactored **Studies** list to use clean, card-based items with badge status indicators.
    - Improved **Settings** page layout: API Key and Storage Directory now span full width for better readability.
    - Modernized **Logs** and **Tools** pages with consistent styling.
- **Theming**:
    - Implemented CSS variables for theme tokens (background, foreground, primary, muted, etc.).
    - Switched global background to a clean slate tone.
- **Backend**:
    - Updated Rust version to `1.80`.
    - Fixed various clippy lints and warnings for better code quality.
