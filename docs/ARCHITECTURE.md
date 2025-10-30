# Architecture Overview

This document describes the high-level architecture of Bounce, a DICOM C-STORE receiver application.

## Table of Contents

- [System Overview](#system-overview)
- [Component Architecture](#component-architecture)
- [Data Flow](#data-flow)
- [Technology Stack](#technology-stack)
- [Module Descriptions](#module-descriptions)

---

## System Overview

Bounce is a hybrid desktop application built with Tauri, combining a Rust backend for high-performance DICOM processing with a modern React/Next.js frontend for user interaction. The application acts as a bridge between on-premises medical imaging equipment and cloud storage.

```
┌─────────────────┐
│  PACS/Modality  │
│   (C-STORE)     │
└────────┬────────┘
         │ DICOM TCP
         │ (Port 104)
         ▼
┌─────────────────────────────────────────────────┐
│              Bounce Application                  │
│  ┌───────────────────────────────────────────┐  │
│  │         Rust Backend (Tauri)              │  │
│  │  ┌──────────────┐  ┌──────────────────┐  │  │
│  │  │    DICOM     │  │   Transmitter    │  │  │
│  │  │   Receiver   │──│   (TUS Upload)   │  │  │
│  │  └──────┬───────┘  └──────────────────┘  │  │
│  │         │                    │            │  │
│  │         │          ┌─────────▼─────────┐ │  │
│  │         │          │   Aurabox API     │ │  │
│  │         │          │     Client        │ │  │
│  │         │          └───────────────────┘ │  │
│  │         │                                 │  │
│  │  ┌──────▼────────┐  ┌─────────────────┐ │  │
│  │  │   SQLite DB   │  │  Config Store   │ │  │
│  │  └───────────────┘  └─────────────────┘ │  │
│  └───────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────┐  │
│  │       Next.js Frontend (WebView)          │  │
│  │  ┌────────┐ ┌─────────┐ ┌──────────────┐ │  │
│  │  │Studies │ │Settings │ │ Logs/Tools   │ │  │
│  │  └────────┘ └─────────┘ └──────────────┘ │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
         │
         │ HTTPS (TUS Protocol)
         │
         ▼
┌─────────────────┐
│  Aurabox Cloud  │
│    Storage      │
└─────────────────┘
```

---

## Component Architecture

### Frontend (Next.js + React)

**Location**: `app/`

The frontend is a server-side rendered Next.js application that provides the user interface. It communicates with the Rust backend via Tauri's IPC (Inter-Process Communication) bridge.

**Key Components**:

- **Page Layout** (`PageLayout.tsx`): Main application shell with navigation
- **Current Status** (`CurrentStatus.tsx`): Real-time server status display
- **Event Handler** (`EventHandler.tsx`): Listens for backend events
- **Settings** (`Settings.tsx`): Configuration management UI
- **Studies Page** (`studies/page.tsx`): Study list and management
- **Logs Page** (`logs/page.tsx`): Application log viewer
- **Tools Page** (`tools/page.tsx`): Utilities and diagnostics

**State Management**:
- Redux Toolkit for global state (`lib/store.ts`)
- Custom hooks for Tauri integration (`lib/customHooks.ts`)

### Backend (Rust + Tauri)

**Location**: `src-tauri/src/`

The backend handles all DICOM operations, file management, database interactions, and cloud communication.

#### Core Modules

##### 1. DICOM Receiver (`receiver/`)

**Purpose**: Implements DICOM C-STORE Service Class Provider (SCP)

**Key Files**:
- `dicom_server.rs`: TCP server and DICOM association handling
- `metadata.rs`: DICOM tag extraction and metadata management
- `enums.rs`: DICOM abstract syntaxes and constants
- `server.rs`: Server lifecycle management

**Functionality**:
- Listens on configured TCP port for DICOM associations
- Handles C-STORE-RQ requests
- Supports C-ECHO for connectivity testing
- Extracts metadata from DICOM objects
- Organizes files by Study UID and Series UID
- Emits events to frontend for real-time updates

##### 2. Transmitter (`transmitter/`)

**Purpose**: Manages study compression and upload to Aurabox

**Key Files**:
- `transmission.rs`: Upload orchestration and study management
- `background.rs`: Background task management

**Functionality**:
- Debounced study aggregation (10-second timer)
- ZIP compression of study folders
- TUS protocol implementation for resumable uploads
- Chunked upload with progress reporting
- Automatic cleanup after successful upload (optional)
- Upload state tracking

##### 3. Aurabox API Client (`aura/`)

**Purpose**: Communication with Aurabox backend

**Key Files**:
- `aura_api.rs`: HTTP client for Aurabox API

**Endpoints**:
- `POST /api/bounce/config`: Fetch upload configuration
- `POST /api/bounce/upload/init`: Initialize upload session
- `POST /api/bounce/upload/start`: Signal upload start
- `POST /api/bounce/upload/complete`: Mark upload complete

##### 4. Database Layer (`db/`)

**Purpose**: SQLite database for persistent storage

**Key Files**:
- `database.rs`: Database connection and queries
- `models.rs`: Data models (Study, Upload, etc.)
- `migrations.rs`: Schema migrations

**Tables**:
- `studies`: Study metadata and status tracking
- `uploads`: Upload history and progress

##### 5. Configuration Store (`store/`)

**Purpose**: Application configuration management

**Key Files**:
- `config.rs`: Configuration loading and persistence

**Configuration Options**:
- API credentials
- DICOM server settings (port, AE title, IP)
- Storage paths
- Upload behavior (auto-delete, etc.)
- Logging preferences

##### 6. Logger (`logger.rs`)

**Purpose**: Centralized logging with remote support

**Features**:
- File-based logging
- Optional remote logging to Better Stack (Logtail)
- Structured log formatting
- Frontend log viewer integration

---

## Data Flow

### Receiving DICOM Files

```
1. DICOM Source (PACS/Modality)
   └─> C-STORE request to Bounce (TCP)
       │
2. DICOMServer::run_store_sync()
   ├─> Establish DICOM association
   ├─> Receive PDUs (Protocol Data Units)
   ├─> Parse DICOM command and data
   ├─> Extract Study UID, Series UID, SOP Instance UID
   │
3. File Storage
   ├─> Create directory: {base_dir}/{study_uid}/{series_uid}/
   ├─> Save DICOM file: {sop_instance_uid}.dcm
   │
4. Metadata Extraction
   ├─> Extract DICOM tags (Patient, Study, Series)
   ├─> Update/create {study_uid}.json metadata file
   │
5. Database Update
   ├─> Insert/update study record in SQLite
   │
6. Event Emission
   └─> Emit "queue-study" event with study_uid
```

### Uploading Studies

```
1. Event Listener ("queue-study")
   └─> Transmission::schedule_study_push()
       │
2. Debounce Logic
   ├─> Start 10-second countdown timer
   ├─> Reset timer if new files for same study arrive
   ├─> Cancel previous scheduled upload
   │
3. Timer Expiry
   └─> Transmission::send_study()
       │
4. Study Compression
   ├─> Walk study directory tree
   ├─> Create ZIP archive: {study_uid}.zip
   │
5. Fetch Upload Config
   ├─> AuraApi::upload_config()
   ├─> Receive TUS endpoint, token, bucket info
   │
6. Initialize Upload
   ├─> AuraApi::upload_init()
   ├─> Send study metadata to Aurabox
   │
7. Upload via TUS
   ├─> Create TUS file (POST request)
   ├─> Upload in 5MB chunks (PATCH requests)
   ├─> Report progress to frontend
   │
8. Complete Upload
   ├─> AuraApi::upload_save("complete")
   ├─> Update study status in database
   │
9. Cleanup (Optional)
   └─> Delete local files if configured
```

---

## Technology Stack

### Backend

- **Runtime**: Rust 1.60+
- **Application Framework**: Tauri 2.4
- **DICOM**: 
  - `dicom` 0.8.0 (Core library)
  - `dicom-ul` 0.8.0 (DICOM Upper Layer)
  - `dicom-transfer-syntax-registry` 0.8.0
- **Async Runtime**: Tokio 1.43 (full features)
- **HTTP Client**: Reqwest 0.12 (with multipart, JSON, streaming)
- **Database**: SQLx 0.8 (SQLite, async)
- **Serialization**: Serde + Serde JSON
- **Compression**: Zip 5.1
- **Logging**: `log` + `tauri-plugin-log`
- **Error Handling**: Anyhow + Snafu

### Frontend

- **Framework**: Next.js 15.0.3 (React 18)
- **UI Components**: Headless UI 2.2
- **Icons**: Heroicons 2.2
- **State Management**: Redux Toolkit 2.4
- **Styling**: Tailwind CSS 3.4
- **Date/Time**: Luxon 3.5
- **Tauri Bindings**: @tauri-apps/api 2.4

### Build Tools

- **Node.js**: 18+
- **Package Manager**: npm
- **Rust Compiler**: Cargo
- **Bundler**: Tauri CLI

---

## Module Descriptions

### `main.rs`

**Purpose**: Application entry point and Tauri setup

**Key Responsibilities**:
- Initialize Tauri runtime
- Set up application state (Database, Transmission, Config)
- Register Tauri commands (IPC handlers)
- Configure event listeners
- Set up logging
- Initialize system tray

**Tauri Commands**:
- `receiver_start`: Start DICOM receiver
- `receiver_stop`: Stop DICOM receiver
- `send_study`: Manually trigger study upload
- `delete_study`: Delete study from local storage
- `reset_app`: Clear all data
- `current_studies`: Fetch study list
- `api_start_upload`: Initialize upload via API
- `show_window`: Show main application window

### `receiver/dicom_server.rs`

**Purpose**: DICOM C-STORE SCP implementation

**Key Methods**:

- `start()`: Bind TCP listener and accept connections
- `run_store_sync()`: Handle individual DICOM association
  - Parse PDUs (Protocol Data Units)
  - Handle C-STORE-RQ commands
  - Handle C-ECHO-RQ commands
  - Extract DICOM metadata
  - Save files to disk
- `extract_string_tag()`: Get string value from DICOM tag
- `create_cstore_response()`: Generate C-STORE-RSP
- `create_cecho_response()`: Generate C-ECHO-RSP

**DICOM Support**:
- All standard transfer syntaxes (uncompressed and compressed)
- Common abstract syntaxes (CT, MR, US, CR, etc.)
- Promiscuous mode for unknown SOP classes

### `transmitter/transmission.rs`

**Purpose**: Upload orchestration

**Key Methods**:

- `schedule_study_push()`: Debounced upload scheduling
- `send_study()`: Main upload workflow
- `compress_study()`: ZIP compression
- `upload_via_tus()`: Chunked TUS upload
- `fetch_uploader_config()`: Get cloud upload config
- `delete_study()`: Remove local study data
- `clear_storage()`: Purge all local data

**Debouncing Logic**:
Uses `tokio::select!` to implement 10-second countdown that resets when new files arrive for the same study. This prevents premature uploads while files are still being received.

### `db/database.rs`

**Purpose**: Database operations

**Key Methods**:

- `new()`: Initialize database connection
- `current_studies()`: Paginated study list
- `delete_study()`: Remove study record
- `clear_studies()`: Truncate studies table

**Schema** (conceptual):
```sql
CREATE TABLE studies (
    id INTEGER PRIMARY KEY,
    study_uid TEXT UNIQUE NOT NULL,
    patient_name TEXT,
    patient_id TEXT,
    study_date TEXT,
    study_description TEXT,
    modality TEXT,
    status TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### `store/config.rs`

**Purpose**: Configuration management

**Configuration Structure**:
```rust
pub struct Config {
    pub api_key: String,
    pub api_endpoint: String,
    pub ae_title: String,
    pub port: u16,
    pub ip_address: String,
    pub storage_dir: String,
    pub delete_after_success: String, // "yes" or "no"
    pub send_logs: String, // "yes" or "no"
}
```

**Storage**: Uses Tauri's Store plugin for persistence

---

## Security Considerations

### Network Security

- DICOM receiver binds to configurable IP (default: all interfaces)
- No authentication on DICOM (standard limitation)
- Cloud uploads use HTTPS only (TLS 1.2+)
- API key authentication for Aurabox communication

### Data Security

- Local files stored with filesystem permissions
- Automatic file deletion option after successful upload
- No plain-text PHI in logs (study UIDs only)
- Secure credential storage via Tauri Store

### Error Handling

- Comprehensive error propagation using `anyhow` and `snafu`
- Failed uploads can be retried manually
- Database transactions ensure data consistency

---

## Performance Characteristics

### Throughput

- **DICOM Receiver**: Handles concurrent connections (one per study/series)
- **Upload Speed**: Limited by network bandwidth (chunked at 5MB)
- **Compression**: Async ZIP compression (doesn't block receiver)

### Resource Usage

- **Memory**: Minimal (files streamed, not loaded entirely into RAM)
- **Disk**: Temporary storage for studies (auto-cleanup available)
- **CPU**: Low when idle, moderate during compression/upload

### Scalability

- Single-threaded DICOM receiver (adequate for typical use cases)
- Async I/O prevents blocking on network operations
- Database pagination for large study lists

---

## Extension Points

### Adding New DICOM Services

Extend `receiver/dicom_server.rs` to handle additional DICOM commands (C-FIND, C-MOVE, etc.)

### Custom Metadata Extraction

Modify `receiver/metadata.rs` to extract additional DICOM tags

### Alternative Storage Backends

Implement storage trait in `transmitter/` to support S3, Azure Blob, etc.

### Authentication

Add authentication layer in `receiver/` or use DICOM TLS

---

## Future Enhancements

- **Multi-destination routing**: Send to multiple cloud providers
- **HL7 integration**: Receive ADT messages for patient context
- **DICOM Query/Retrieve**: Act as C-FIND/C-MOVE SCP
- **DICOM TLS**: Secure DICOM communications
- **Web UI without Tauri**: Optional web-based management interface
- **Docker deployment**: Containerized deployment option
- **Study anonymization**: Built-in de-identification

---

## References

- [DICOM Standard](https://www.dicomstandard.org/)
- [Tauri Documentation](https://tauri.app/v1/guides/)
- [TUS Protocol Specification](https://tus.io/protocols/resumable-upload.html)
- [Rust DICOM Documentation](https://docs.rs/dicom/)
