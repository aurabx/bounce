# Changelog

All notable changes to this project will be documented in this file.

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
