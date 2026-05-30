# Development Guide

This guide provides detailed instructions for developers contributing to or working with the Bounce codebase.

## Table of Contents

- [Makefile Quick Reference](#makefile-quick-reference)
- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Building](#building)
- [Testing](#testing)
- [Debugging](#debugging)
- [Code Style](#code-style)
- [Common Tasks](#common-tasks)
- [Troubleshooting](#troubleshooting)

---

## Makefile Quick Reference

A `Makefile` at the project root wraps the most common npm and cargo commands. Run `make help` for the full list.

| Command | Description |
|---|---|
| `make dev` | Start full Tauri app with hot-reload |
| `make dev-frontend` | Start Next.js frontend only |
| `make dev-backend` | Run Rust backend only |
| `make build` | Build frontend + Tauri release bundle |
| `make test` | Run all tests |
| `make lint` | Run all linters (ESLint + Clippy) |
| `make fmt` | Format all code |
| `make check` | Lint + format check + tests (pre-commit) |
| `make clean` | Remove build artifacts |
| `make install` | Install Node + Rust dependencies |
| `make version V=x.y.z` | Bump version everywhere |
| `make dicom-echo` | DICOM C-ECHO connectivity test |
| `make dicom-send FILE=f.dcm` | Send a DICOM file via C-STORE |

---

## Getting Started

### Prerequisites

Before you begin, ensure you have the following installed:

#### Required

- **Node.js** 18+ and npm
  ```bash
  node --version  # Should be 18.x or higher
  npm --version
  ```

- **Rust** (latest stable) and Cargo
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustc --version  # Should be 1.60 or higher
  ```

- **Tauri CLI** (installed via npm)
  ```bash
  npm install --save-dev @tauri-apps/cli
  ```

#### Platform-Specific Dependencies

**Linux** (Ubuntu/Debian):
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libsqlite3-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

**macOS**:
```bash
xcode-select --install
```

**Windows**:
- Install [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (usually pre-installed on Windows 11)

### Initial Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/aurabx/bounce.git
   cd bounce
   ```

2. **Install Node dependencies**:
   ```bash
   npm install
   ```

3. **Build Rust dependencies** (optional, happens automatically):
   ```bash
   cd src-tauri
   cargo build
   cd ..
   ```

4. **Run the application in development mode**:
   ```bash
   make dev
   ```

The application will launch with hot-reload enabled for both frontend and backend changes.

> **Tip**: Run `make help` to see all available Make targets.

---

## Development Environment

### Recommended IDE Setup

#### Visual Studio Code

**Extensions**:
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) - Rust language support
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) - Tauri development tools
- [ESLint](https://marketplace.visualstudio.com/items?itemName=dbaeumer.vscode-eslint) - JavaScript/TypeScript linting
- [Prettier](https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode) - Code formatting
- [Tailwind CSS IntelliSense](https://marketplace.visualstudio.com/items?itemName=bradlc.vscode-tailwindcss) - Tailwind autocomplete

**Workspace Settings** (`.vscode/settings.json`):
```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.eslint": true
  },
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "[javascript]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[typescript]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  },
  "[typescriptreact]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  }
}
```

#### IntelliJ IDEA / CLion

- Install Rust plugin
- Install JavaScript plugin
- Configure Cargo to use nightly (optional)

---

## Development Workflow

### Using the Makefile

A `Makefile` is provided at the project root with targets for all common development tasks. Run `make help` to see the full list.

### Running the Development Server

```bash
# Start Tauri app with hot-reload (recommended)
make dev

# Or, run frontend and backend separately:
make dev-frontend    # Frontend only (Next.js)
make dev-backend     # Backend only (Rust)
```

The equivalent npm/cargo commands still work if you prefer them:

```bash
npm run tauri:dev
npm run dev
cargo run --manifest-path=src-tauri/Cargo.toml
```

### Making Changes

#### Frontend Changes

1. Edit files in `app/` directory
2. Changes will hot-reload automatically
3. Check browser console for errors
4. Use React DevTools for component debugging

#### Backend Changes

1. Edit files in `src-tauri/src/`
2. Save the file
3. Backend will recompile and restart automatically
4. Check terminal output for compile errors

### Adding Dependencies

**Frontend (npm)**:
```bash
npm install <package-name>
npm install --save-dev <dev-package-name>
```

**Backend (Cargo)**:
```bash
cd src-tauri
cargo add <crate-name>
cargo add --dev <dev-crate-name>
cd ..
```

---

## Building

### Development Build

```bash
make dev
```

### Production Build

```bash
# Build frontend + Tauri app in one step
make build

# Or individually:
make build-frontend   # Next.js static export only
make build-release    # Full Tauri application bundle
```

Output locations:
- **macOS**: `src-tauri/target/release/bundle/dmg/`
- **Windows**: `src-tauri/target/release/bundle/msi/`
- **Linux**: `src-tauri/target/release/bundle/deb/` or `appimage/`

Signed and notarized release builds are produced by CI, not locally.
See [`RELEASE-SIGNING.md`](RELEASE-SIGNING.md) for the codesigning,
notarization, and Tauri updater key setup, including how to rotate
each secret and renew the Apple Developer ID certificate.

### Build for Specific Platform

```bash
# Build for current platform only
npm run tauri:build

# Build with custom target (advanced)
cd src-tauri
cargo build --release --target x86_64-pc-windows-msvc
```

---

## Testing

### Frontend Tests

Currently, the project doesn't have a test suite set up. To add testing:

```bash
npm install --save-dev @testing-library/react @testing-library/jest-dom jest
```

### Backend Tests

**Unit tests**:
```bash
make test-rust

# Or run all tests:
make test
```

**Integration tests**: Add to `src-tauri/tests/`

### Manual Testing

#### Test DICOM Receiver

1. Start the app in dev mode
2. Configure settings (API key, port, etc.)
3. Start the DICOM server
4. Use DCMTK to send test files:

```bash
# C-ECHO (connectivity test)
make dicom-echo PORT=12345

# C-STORE (send DICOM file)
make dicom-send FILE=test.dcm PORT=12345

# Or using DCMTK directly:
echoscu -v -aec BOUNCE localhost 12345
storescu -v -aec BOUNCE localhost 12345 test.dcm
```

#### Test File Upload

1. Send DICOM files via C-STORE
2. Wait 10 seconds (debounce period)
3. Check Studies page for upload status
4. Verify file appears in Aurabox

---

## Debugging

### Frontend Debugging

**Chrome DevTools**:
- Open the application
- Right-click → "Inspect Element"
- Use Console, Network, and React DevTools tabs

**Console Logging**:
```typescript
console.log('Debug info:', variable);
```

### Backend Debugging

**Print Debugging**:
```rust
println!("Debug: {:?}", value);
```

**Logging**:
```rust
use crate::{log_info, log_error};

log_info!("Server started on port {}", port);
log_error!("Failed to connect: {}", error);
```

**Rust Debugger (LLDB/GDB)**:

Add to `src-tauri/.cargo/config.toml`:
```toml
[build]
target-dir = "target"

[profile.dev]
split-debuginfo = "unpacked"
```

Then use VS Code's CodeLLDB extension or command line:
```bash
rust-lldb target/debug/app
```

**Check Logs**:
- Application logs are written to: `~/.local/share/com.aurabox.bounce/logs/` (Linux)
- Or: `~/Library/Application Support/com.aurabox.bounce/logs/` (macOS)
- Or: `%APPDATA%\com.aurabox.bounce\logs\` (Windows)

---

## Code Style

### Rust Code Style

Follow standard Rust conventions:

```bash
# Format code
make fmt-rust

# Lint code
make lint-rust

# Check formatting without modifying files
make fmt-check

# Run all linters (frontend + backend)
make lint

# Run pre-commit checks (lint + format check + tests)
make check
```

**Conventions**:
- Use `snake_case` for functions and variables
- Use `PascalCase` for types and traits
- Add documentation comments (`///`) for public APIs
- Use `Result` and `?` for error handling
- Prefer `async/await` over callbacks

**Example**:
```rust
/// Processes a DICOM study and uploads it to the cloud.
///
/// # Arguments
/// * `study_uid` - The unique identifier for the study
///
/// # Returns
/// * `Ok(())` if successful
/// * `Err` if upload fails
pub async fn process_study(study_uid: String) -> Result<()> {
    let study = load_study(&study_uid).await?;
    let archive = compress_study(&study).await?;
    upload_archive(&archive).await?;
    Ok(())
}
```

### TypeScript/React Code Style

```bash
# Lint code
make lint-frontend
```

**Conventions**:
- Use `camelCase` for variables and functions
- Use `PascalCase` for components and types
- Use functional components with hooks
- Add JSDoc comments for complex functions
- Prefer `const` over `let`

**Example**:
```typescript
/**
 * Displays the current status of the DICOM receiver
 */
export function CurrentStatus() {
  const [status, setStatus] = useState<string>('stopped');
  
  useEffect(() => {
    // Subscribe to status events
    const unlisten = listen('server-status', (event) => {
      setStatus(event.payload as string);
    });
    
    return () => { unlisten(); };
  }, []);
  
  return (
    <div className="status-widget">
      <span>Status: {status}</span>
    </div>
  );
}
```

---

## Common Tasks

### Adding a New Tauri Command

1. **Define the command in `src-tauri/src/main.rs`**:

```rust
#[tauri::command]
async fn my_new_command(app: AppHandle, param: String) -> Result<String, String> {
    println!("Received: {}", param);
    Ok(format!("Processed: {}", param))
}
```

2. **Register the command**:

```rust
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            receiver_start,
            receiver_stop,
            my_new_command,  // Add here
            // ... other commands
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

3. **Call from frontend**:

```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<string>('my_new_command', { param: 'test' });
console.log(result);
```

### Adding a New Page

1. **Create page file**: `app/my-page/page.tsx`

```typescript
export default function MyPage() {
  return (
    <div>
      <h1>My New Page</h1>
    </div>
  );
}
```

2. **Add to menu**: Update `app/lib/menu.ts`

```typescript
export const menuItems = [
  // ... existing items
  { name: 'My Page', href: '/my-page', icon: DocumentIcon },
];
```

### Adding a New Database Table

1. **Create migration in `src-tauri/src/db/migrations.rs`**:

```rust
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS my_table (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )"
    )
    .execute(pool)
    .await?;
    
    Ok(())
}
```

2. **Add model in `src-tauri/src/db/models.rs`**:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct MyModel {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}
```

3. **Add queries in `src-tauri/src/db/database.rs`**:

```rust
impl Database {
    pub async fn insert_my_model(&self, name: String) -> Result<i64> {
        let result = sqlx::query("INSERT INTO my_table (name) VALUES (?)")
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(result.last_insert_rowid())
    }
}
```

### Updating Application Version

```bash
make version V=1.2.3
```

Or use the script directly:

```bash
./update-version.sh 1.2.3
```

This updates:
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

---

## Troubleshooting

### Build Fails with Rust Errors

**Problem**: Cargo build fails with dependency errors

**Solution**:
```bash
make clean-rust
cd src-tauri && cargo update
make build-release
```

### Frontend Hot Reload Not Working

**Problem**: Changes to React components don't reflect

**Solution**:
1. Stop the dev server (Ctrl+C)
2. Clear caches: `make clean-frontend`
3. Restart: `make dev`

### DICOM Server Won't Bind to Port

**Problem**: "Address already in use" error

**Solution**:
```bash
# Check what's using port 12345
sudo lsof -i :12345

# Kill the process if needed
sudo kill -9 <PID>

# Or use a different port in Settings
```

### WebView Not Loading on Linux

**Problem**: Blank window on Linux

**Solution**:
```bash
# Install WebKit dependencies
sudo apt install webkit2gtk-4.1-dev

# If still failing, check:
ldd src-tauri/target/debug/app
```

### Database Migration Fails

**Problem**: SQLite errors on startup

**Solution**:
```bash
# Delete database and let it recreate
rm ~/.local/share/com.aurabox.bounce/bounce.db

# Restart app
```

### TypeScript Type Errors

**Problem**: Type mismatches in frontend code

**Solution**:
```bash
# Regenerate types
npm run build

# Or ignore temporarily (not recommended)
// @ts-ignore
```

---

## CI/CD

### GitHub Actions (Example)

Create `.github/workflows/build.yml`:

```yaml
name: Build

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  build:
    strategy:
      matrix:
        platform: [ubuntu-latest, windows-latest, macos-latest]
    
    runs-on: ${{ matrix.platform }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: 18
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Install dependencies (Ubuntu)
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt update
          sudo apt install -y libwebkit2gtk-4.1-dev libssl-dev libsqlite3-dev
      
      - name: Install npm dependencies
        run: npm install
      
      - name: Build frontend
        run: npm run build
      
      - name: Build Tauri app
        run: npm run tauri:build
```

---

## Resources

- [Tauri Documentation](https://tauri.app/v1/guides/)
- [Next.js Documentation](https://nextjs.org/docs)
- [Rust Book](https://doc.rust-lang.org/book/)
- [DICOM Standard](https://www.dicomstandard.org/)
- [TUS Protocol](https://tus.io/)

---

## Getting Help

If you encounter issues not covered here:

1. Check existing [GitHub Issues](https://github.com/aurabx/bounce/issues)
2. Review application logs
3. Search [Tauri Discord](https://discord.com/invite/tauri)
4. Contact the team: dev@aurabox.cloud
