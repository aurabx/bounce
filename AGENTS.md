# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

Bounce is a cross-platform desktop DICOM C-STORE receiver built with Tauri 2.4 (Rust backend) and Next.js 15 (React frontend). It receives medical imaging files from PACS/modalities via DICOM protocol and securely uploads them to Aurabox cloud storage using the TUS resumable upload protocol.

## Essential Development Commands

### Development
```bash
# Run full application in development mode with hot-reload
npm run tauri:dev

# Run frontend only (Next.js dev server)
npm run dev

# Run backend only (Rust)
cargo run --manifest-path=src-tauri/Cargo.toml
```

### Building
```bash
# Build Next.js frontend only
npm run build

# Build complete Tauri application for release
npm run tauri:build

# Build Rust backend only
cd src-tauri && cargo build --release
```

### Testing
```bash
# Run Rust unit tests
cd src-tauri && cargo test

# Lint frontend
npm run lint

# Format Rust code
cd src-tauri && cargo fmt

# Run Rust linter
cd src-tauri && cargo clippy
```

### DICOM Testing
```bash
# Test DICOM connectivity (C-ECHO)
echoscu -v -aec BOUNCE localhost 104

# Send a single DICOM file (C-STORE)
storescu -v -aec BOUNCE localhost 104 /path/to/file.dcm

# Send all DICOM files in a directory
storescu -v -aec BOUNCE localhost 104 /path/to/dicom/folder/*.dcm
```

### Version Management
```bash
# Update version across package.json, Cargo.toml, and tauri.conf.json
./update-version.sh 1.2.3
```

## Architecture Overview

### Hybrid Tauri Application
- **Frontend**: Next.js 15 with React 18, exported as static files (`output: 'export'`)
- **Backend**: Rust with Tokio async runtime
- **IPC**: Tauri commands bridge frontend ↔ backend
- **State Management**: Redux Toolkit (frontend), Tauri managed state (backend)

### Critical Backend Modules

#### 1. DICOM Receiver (`src-tauri/src/receiver/`)
- `dicom_server.rs`: Implements DICOM C-STORE SCP (Service Class Provider)
- `server.rs`: Server lifecycle management (start/stop)
- `metadata.rs`: Extracts DICOM tags and creates JSON metadata files
- Handles TCP connections on configurable port (default: 104)
- Supports C-ECHO for connectivity testing and C-STORE for receiving files
- Files organized by: `{storage_dir}/{study_uid}/{series_uid}/{sop_instance_uid}.dcm`

#### 2. Transmitter (`src-tauri/src/transmitter/`)
- `transmission.rs`: Upload orchestration with 10-second debouncing
- `background.rs`: Background task management
- Waits 10 seconds after last file received before compressing/uploading study
- Compresses study folder to ZIP, uploads via TUS protocol in 5MB chunks
- Emits progress events to frontend

#### 3. Aurabox API Client (`src-tauri/src/aura/`)
- `aura_api.rs`: HTTP client for Aurabox backend
- Endpoints: `/api/bounce/config`, `/api/bounce/upload/init`, `/api/bounce/upload/start`, `/api/bounce/upload/complete`

#### 4. Database (`src-tauri/src/db/`)
- SQLite via SQLx (async)
- `database.rs`: Connection pool and queries
- `models.rs`: Data models (Study, Upload)
- `migrations.rs`: Schema migrations
- Tracks study metadata, upload status, and transmission history

#### 5. Configuration (`src-tauri/src/store/`)
- `config.rs`: Manages app settings via Tauri Store plugin
- Config includes: API key, AE title, port, IP address, storage path, auto-delete setting

### Key Tauri Commands (IPC)
These functions in `src-tauri/src/main.rs` are callable from the frontend:
- `receiver_start`: Start DICOM receiver
- `receiver_stop`: Stop DICOM receiver
- `send_study`: Manually trigger study upload
- `delete_study`: Delete study from local storage and database
- `reset_app`: Clear all data
- `current_studies`: Fetch paginated study list
- `api_start_upload`: Initialize upload session with Aurabox
- `show_window`: Show main application window

### Frontend Structure (`app/`)
- `components/`: Reusable React components
  - `CurrentStatus.tsx`: Real-time server status display
  - `EventHandler.tsx`: Listens to backend events via Tauri
  - `Settings.tsx`: Configuration form
  - `PageLayout.tsx`: Application shell with navigation
- `studies/page.tsx`: Study list and management
- `logs/page.tsx`: Application log viewer
- `tools/page.tsx`: Utilities and diagnostics
- `lib/store.ts`: Redux store configuration
- `lib/customHooks.ts`: Custom hooks for Tauri integration

## Critical Dependencies

### Backend (Rust)
- `tauri` 2.4.0: Desktop application framework
- `tokio` 1.43: Async runtime (full features)
- `dicom` 0.8.0, `dicom-ul` 0.8.0: DICOM protocol implementation
- `sqlx` 0.8: Async SQLite database
- `reqwest` 0.12: HTTP client with streaming, multipart support
- `zip` 5.1: ZIP compression

### Frontend (TypeScript)
- `next` 15.1.2: React framework (static export mode)
- `@tauri-apps/api` 2.4.0: Tauri JavaScript bindings
- `@reduxjs/toolkit` 2.4.0: State management
- `@headlessui/react` 2.2.0: Unstyled UI components
- `tailwindcss` 3.4: Utility-first CSS

## Development Patterns

### Adding a New Tauri Command
1. Define function in `src-tauri/src/main.rs` with `#[tauri::command]` attribute
2. Register in `invoke_handler!` macro in `main()`
3. Call from frontend: `invoke<ReturnType>('command_name', { param: value })`

### Event Communication
Backend emits events to frontend:
```rust
app.emit("event-name", payload).unwrap();
```
Frontend listens:
```typescript
import { listen } from '@tauri-apps/api/event';
listen<PayloadType>('event-name', (event) => { /* handle */ });
```

### Database Queries
Use async SQLx with the managed Database state:
```rust
let database = app.state::<Database>();
let studies = database.current_studies(page, limit).await;
```

### Configuration Access
```rust
let config = Config::load(app_handle.clone());
// Access config.api_key, config.port, etc.
```

## Build and Release Process

### Release Workflow
- Triggered by pushing to `release` branch
- Workflow: `.github/workflows/publish.yml`
- Creates draft GitHub release with tag `aurabox-bounce-v{version}`
- Builds for: macOS (Intel & ARM), Linux (Ubuntu 22.04), Windows
- macOS builds require Apple Developer certificate secrets
- Uses `yarn install` (note: package-lock.json exists, but workflow uses yarn)

### Platform-Specific Outputs
- **macOS**: `.dmg` in `src-tauri/target/release/bundle/dmg/`
- **Windows**: `.msi` in `src-tauri/target/release/bundle/msi/`
- **Linux**: `.deb`, `.AppImage` in `src-tauri/target/release/bundle/`

### Signing and Notarization
- macOS: Uses signing identity "Developer ID Application: Aurabox Pty Ltd (X52XM4SP3U)"
- Requires secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_ID_PASSWORD`, `APPLE_TEAM_ID`

## Important Constraints and Gotchas

### Next.js Configuration
- Uses `output: 'export'` for static site generation (no server-side rendering)
- Frontend is compiled to `out/` directory
- No API routes or dynamic server features available

### DICOM Server
- Port 104 requires elevated privileges on Linux/macOS (`sudo` may be needed)
- Single-threaded DICOM receiver (adequate for typical use cases)
- No DICOM authentication (standard limitation of C-STORE protocol)

### Upload Debouncing
- Studies are automatically uploaded 10 seconds after the last file is received
- Timer resets if new files for the same study arrive
- Prevents premature uploads while files are still streaming in

### Database Location
- Linux: `~/.local/share/com.aurabox.bounce/`
- macOS: `~/Library/Application Support/com.aurabox.bounce/`
- Windows: `%APPDATA%\com.aurabox.bounce\`

### Log File Location
Logs are written by `tauri-plugin-log` to the platform-specific log directory (not the database/app data directory):
- Linux: `~/.local/share/com.aurabox.bounce/logs/logs.log`
- macOS: `~/Library/Logs/com.aurabox.bounce/logs.log`
- Windows: `%APPDATA%\com.aurabox.bounce\logs\logs.log`

The log filename `logs.log` is configured in `main.rs` via `file_name: Some("logs".to_string())`.

Remote logging to Better Stack (Logtail) is also available when enabled via config.

## Common Development Tasks

### Testing DICOM Reception
1. Start app: `npm run tauri:dev`
2. Configure settings (API key is required for uploads)
3. Start DICOM server from UI
4. Use `storescu` from DCMTK to send test files
5. Check Studies page for upload status

### Debugging Backend
- Use `log_info!`, `log_error!` macros defined in `logger.rs`
- Check terminal output during `tauri:dev`
- Logs written to platform-specific log directory (see Log File Location above)

### Debugging Frontend
- Open Chrome DevTools: right-click → "Inspect Element"
- Backend events visible in Redux DevTools
- Use `console.log()` for debugging

### Clearing All Data
- Use "Reset App" button in UI (calls `reset_app` Tauri command)
- Or manually delete database file and storage directory

## Security Considerations

- All cloud uploads use HTTPS (TLS 1.2+)
- API key authentication for Aurabox communication
- PHI (Protected Health Information) not logged in plain text
- Local files protected by filesystem permissions
- Optional automatic deletion after successful upload
