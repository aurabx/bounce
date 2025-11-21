# Changelog

All notable changes to this project will be documented in this file.

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
