# Bounce - AI Coding Agent Instructions

## Project Overview

**Bounce** is a cross-platform desktop application that serves as a DICOM C-STORE receiver, securely forwarding medical imaging files to the Aurabox cloud platform. It bridges on-premises medical equipment with cloud-based DICOM storage.

**Primary Technologies:**
- **Frontend**: Next.js 15 (React 18) with TypeScript, TailwindCSS, Redux Toolkit
- **Backend**: Rust with Tauri 2.x framework
- **Database**: SQLite for study tracking and metadata persistence
- **Protocols**: DICOM C-STORE/C-FIND (medical imaging), TUS (resumable uploads)
- **Desktop**: Tauri for cross-platform native app (Windows, macOS, Linux)

The application runs behind healthcare firewalls, receives DICOM files from PACS/modalities, extracts metadata, compresses studies, and uploads them to Aurabox over HTTPS with resumable upload capability.

## Build & Run Commands

**Development:**
- `make dev` or `npm run tauri:dev` — Start full Tauri app with hot-reload (backend + frontend)
- `make dev-frontend` or `npm run dev` — Next.js frontend only (port 3000)
- `make dev-backend` — Run Rust backend without Tauri window manager

**Building:**
- `make build` — Build Next.js frontend then full Tauri release app
- `make build-frontend` or `npm run build` — Next.js static export only
- `npm run tauri:build` — Full Tauri application bundle for release

**Testing:**
- `make test` or `make test-rust` — Run Rust unit tests (cd src-tauri && cargo test)
- `make dicom-echo` — Test DICOM connectivity with C-ECHO (requires DCMTK)
- `make dicom-send FILE=path.dcm` — Send test DICOM file via C-STORE

**Quality:**
- `make lint` — Run ESLint (frontend) and Clippy (backend)
- `make fmt` — Format Rust code with cargo fmt
- `make check` — Run linters + format check + tests (pre-commit)

**Utilities:**
- `make install` — Install npm dependencies and fetch Rust crates
- `make version V=x.y.z` — Update version across package.json, Cargo.toml, tauri.conf.json
- `make clean` — Remove build artifacts (frontend + backend)

## Architecture Overview

**Directory Structure:**

- **`app/`** — Next.js 15 frontend application (App Router)
  - `components/` — React UI components (Sidebar, PageLayout, CurrentStatus, Settings, etc.)
  - `lib/` — Frontend utilities, Redux store, Tauri hooks, type definitions, helpers
  - `page.tsx`, `layout.tsx` — Root page and app layout
  - `studies/`, `logs/`, `settings/`, `tools/` — Feature pages

- **`src-tauri/src/`** — Rust backend (Tauri application)
  - `main.rs` — Entry point, initializes Tauri runtime, logging, database
  - `receiver/` — DICOM C-STORE SCP implementation (receives files from PACS)
  - `transmitter/` — TUS protocol upload logic (sends compressed studies to Aurabox)
  - `query/` — DICOM C-FIND SCP implementation (queries remote PACS)
  - `aura/` — Aurabox API client (authentication, upload coordination)
  - `db/` — SQLite database abstraction for study tracking
  - `store/` — Settings persistence using Tauri plugin-store
  - `logger.rs` — Structured logging with tracing crate, optional remote logging

- **`docs/`** — Technical documentation (API, architecture, configuration, development guides)

- **`docker/`** — Orthanc PACS test configuration (docker-compose.yml for local testing)

**Key Data Flows:**

1. **DICOM Reception**: PACS → C-STORE receiver → SQLite + disk storage → metadata extraction
2. **Study Aggregation**: Debouncing logic groups instances into studies by Study UID
3. **Upload Pipeline**: Study compression → TUS chunked upload → Aurabox cloud platform
4. **IPC Communication**: Rust backend emits Tauri events → EventHandler → Redux store → React UI updates
5. **User Configuration**: Settings UI → Tauri store plugin → Rust backend reloads config

## Coding Conventions

**Rust (Backend):**
- **Naming**: `snake_case` for functions/variables, `PascalCase` for structs/enums, `SCREAMING_SNAKE_CASE` for constants
- **Error Handling**: All functions return `Result<T, E>`. Never `unwrap()` in production code without justification; prefer `?` operator or explicit error context
- **Async**: Use `tokio` runtime with `async`/`await`. All async functions must have explicit signatures
- **Types**: Explicit type annotations for public APIs. Use strong typing with `serde` for serialization
- **Formatting**: `cargo fmt` is canonical. Run `make fmt-check` before committing

**TypeScript (Frontend):**
- **Naming**: `PascalCase` for components/types, `camelCase` for functions/variables
- **Components**: Prefer function components over classes. Use `.tsx` extension for React components
- **Styling**: TailwindCSS utility classes only. No inline styles or CSS modules
- **State**: Redux Toolkit for global state. Tauri hooks (`useStore`, `useTauriEvent`) for backend integration
- **Error Handling**: Throw errors with descriptive messages. Catch at component boundaries with error states
- **Types**: Explicit return types for complex functions. Use TypeScript strict mode

**General Patterns:**
- **Version Sync**: `package.json`, `Cargo.toml`, `tauri.conf.json` must stay synchronized. Use `make version V=x.y.z`
- **DICOM UIDs**: Case-sensitive strings. Compare exactly as received without normalization
- **File Paths**: Use Tauri path APIs (`app.resolveResource()`, etc.) for cross-platform compatibility
- **Logging**: Use structured logging in Rust (`tracing` crate). Include context (study UID, AE title, etc.)

## Agent Guidance

**Always Do:**
- Run `make test` before committing Rust code changes
- Validate DICOM protocol changes against Orthanc test PACS (`docker-compose up`) or DCMTK tools
- Update version atomically with `make version V=x.y.z` when incrementing releases
- Use Tauri's async `invoke()` for all frontend-to-backend calls (returns Promises)
- Check error logs in `~/.aurabox/bounce/logs/` when debugging backend issues
- Add structured logging with context for new backend features
- Batch SQLite transactions to avoid blocking the DICOM receiver thread
- Handle TUS upload resumption from last chunk offset (not restart)

**Never Do:**
- Commit secrets, API keys, or credentials (even in examples)
- Use `unwrap()` in production Rust code without explicit safety justification
- Hardcode file paths with platform-specific separators (use Tauri path APIs)
- Assume PACS implementations strictly follow DICOM standard (handle malformed data gracefully)
- Break version synchronization across `package.json`, `Cargo.toml`, `tauri.conf.json`
- Delete database files or study directories without user confirmation
- Modify the proprietary license (UNLICENSED) without authorization

**Ask Before:**
- Deleting files from disk (studies, database, logs)
- Changing default DICOM network parameters (port 104, AE title "BOUNCE")
- Adding new npm or cargo dependencies (check license compatibility with proprietary project)
- Modifying database schema (requires migration strategy)
- Changing TUS upload chunking behavior (affects resumability)
- Altering DICOM metadata extraction logic (medical imaging compliance)

**Testing Protocol:**
- DICOM features: Test with Orthanc PACS (port 4242) or `storescu`/`echoscu` from DCMTK
- Frontend changes: Verify in all three platforms (Windows, macOS, Linux) if possible
- Upload pipeline: Test with large studies (>100MB) to validate resumability
- Error scenarios: Test network failures, invalid DICOM files, API key rejection, disk full

**Documentation Updates:**
- Update `docs/API.md` when adding/modifying Tauri commands or events
- Update `CHANGELOG.md` with user-facing changes (features, fixes, breaking changes)
- Add inline code comments for complex DICOM protocol handling or medical imaging logic
- Update `docs/CONFIGURATION.md` when adding new user settings

<!-- automatic:groups:start -->
## Related Projects
The following projects are related to this one. They are provided for context — explore or reference them when relevant to the current task.

### Aura
The Aurabox application and related projects
**aura**
Location: `../aura`
**lasso**
Location: `../lasso`
**uhura**
Location: `../uhura`
**lift**
Location: `../lift`
**starfleet**
Location: `../starfleet`
**skills**
Location: `../skills`
**ravana**
Location: `../../_experiments/ravana`
**tus-server**
Location: `../tus-server`
**cloud-lib-gcp**
Location: `../cloud-lib-gcp`
**gcp-pub-sub**
Location: `../gcp-pub-sub`
**scanfinder**
Location: `../scanfinder`
**nightowl**
Location: `../../_experiments/nightowl`
**meridian**
Location: `../../_experiments/meridian`

<!-- automatic:groups:end -->
