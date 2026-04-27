# Architecture Overview

This document describes the high-level architecture of Bounce, a DICOM C-STORE receiver application.

## Table of Contents

- [System Overview](#system-overview)
- [Component Architecture](#component-architecture)
- [Data Flow](#data-flow)
- [Technology Stack](#technology-stack)
- [Module Descriptions](#module-descriptions)
- [Security Considerations](#security-considerations)
- [Performance Characteristics](#performance-characteristics)
- [Extension Points](#extension-points)
- [Future Enhancements](#future-enhancements)

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

### Outbound Send (Aurabox → remote PACS, C-STORE SCU)

The send pipeline is the inverse of the receive pipeline: instead of accepting
images from a PACS and uploading them to Aurabox, Bounce fetches a study held in
Aurabox and pushes it via DIMSE to a destination PACS.

```
1. Aura queues a PacsSend (study_uid + destination AE + WADO source/JWT)
   │
2. Bounce poll cycle (query/poller.rs)
   ├─> AuraApi::fetch_pending_jobs()  ← unified jobs endpoint
   ├─> Split into retrieves (sequential) and sends (semaphore-bounded)
   │
3. Per send (send/worker.rs, capped at 2 concurrent per gateway)
   ├─> WADO-RS GET {wado_base}{study_path}
   │     - Accept: multipart/related; type=application/dicom
   │     - Authorization: Bearer {study-scoped JWT minted by Aura}
   │     - Body parsed via send/uhura_client.rs into FileDicomObjects
   │
   ├─> Optional series filter (job.series_uids)
   │
   ├─> AuraApi::post_send_progress(0, total)  ← initial UI signal
   │
   ├─> query/cstore.rs::execute_cstore()
   │     - Open association proposing one PC per distinct SOP class
   │     - Transfer syntaxes offered: Explicit VR LE, Implicit VR LE,
   │       JPEG Baseline (Process 1)
   │     - For each instance:
   │         · build C-STORE-RQ command
   │         · serialize dataset with the negotiated TS for the matching PC
   │         · send Command + Data PDUs
   │         · receive C-STORE-RSP, classify status as success / warning / error
   │     - Returns a SendReport (successes + failures)
   │
   ├─> If all instances stored → AuraApi::post_send_completed()
   └─> Otherwise              → AuraApi::post_send_failed(summary)
```

The send path treats a job as atomic for v1: any instance-level failure is
reported as a send-level failure with the first failing instance's status code
and ErrorComment surfaced. Mid-send TCP recovery, on-the-fly transcoding, and
sub-series-level granularity are deferred — see Future Enhancements.

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

This section documents the concurrency model, hard limits, and known
bottlenecks observed in the current implementation. Numbers and file
references reflect the code as of writing — verify against the source
before relying on them for capacity planning.

### DICOM Reception (Inbound)

| Aspect                       | Limit                              | Source                                                              |
|------------------------------|------------------------------------|---------------------------------------------------------------------|
| Concurrent associations      | No explicit cap                    | Bare `listener.accept()` loop in `src-tauri/src/receiver/dicom_server.rs` |
| Per-association processing   | Serialized (one PDU at a time)     | `run_store_sync()` is blocking; one DICOM op at a time per connection |
| TCP backlog                  | OS default (~128 on Linux/macOS)   | `listener.bind()` does not set an explicit backlog                  |
| Tokio worker threads         | `num_cpus` (Tokio default)         | `tokio = { features = ["full"] }` in `src-tauri/Cargo.toml`         |
| File descriptors             | OS `ulimit -n` (typically 1024–10240) | Not raised by the application                                    |

**Practical ceiling.** Reception is bounded by `min(OS file descriptors,
TCP backlog backpressure)`. The application does not enforce a maximum
number of concurrent associations, so a misbehaving or hostile peer can
open associations until the OS file-descriptor limit is exhausted. Only
`num_cpus` associations can perform CPU-bound work concurrently; the
rest queue on Tokio workers.

### TUS Upload (Outbound)

| Aspect                  | Limit                  | Source                                                                 |
|-------------------------|------------------------|------------------------------------------------------------------------|
| Concurrent uploads      | **1 study at a time**  | Single `reqwest::Client`, sequential `send_study()` in `src-tauri/src/transmitter/transmission.rs` |
| Chunk size              | **5 MB** (hardcoded)   | `src-tauri/src/transmitter/transmission.rs`                            |
| Study debounce          | **10 seconds** (hardcoded) | `src-tauri/src/transmitter/transmission.rs`                        |
| HTTP connection pool    | reqwest default (~32)  | `Client::new()` is unconfigured                                        |

**Practical ceiling.** Upload throughput is dominated by the
single-study, sequential-chunk model. Sustained MB/s per study is
approximately `5 MB / (network RTT + server-ack time)`. End-to-end
study cadence is `≥ debounce + (study size / effective link speed)`.
Uploads are the dominant bottleneck for end-to-end throughput.

### Database

- SQLite is opened via `SqlitePool::connect()` in
  `src-tauri/src/db/database.rs` with no explicit pool size and no WAL
  mode set.
- Each received DICOM instance triggers an individual
  `create_or_update_study()` call; writes are not batched.
- SQLite serializes writes globally, so high-rate inbound DICOM
  produces lock contention on the receiver path.

### Resource Usage

- **Memory**: Minimal — files are streamed to disk and read back in
  chunks for upload; full studies are not held in memory.
- **Disk**: Temporary storage per study under the configured base
  directory; optional auto-cleanup via `delete_after_success`.
- **CPU**: Low when idle, moderate during ZIP compression and TLS
  encryption of outbound chunks.

### Configurable Limits

There are currently **no user-configurable knobs** for concurrency,
chunk size, pool sizes, or association caps. The settings surface
exposes only port, IP address, AE title, base directory,
`delete_after_success`, and `send_logs`.

### Known Bottlenecks and Mitigation Candidates

The following are recognised limitations rather than defects. Address
them only when motivated by a concrete capacity requirement:

1. **Single-flight uploads.** A configurable upload concurrency knob
   backed by a `Semaphore` would allow parallel TUS sessions when
   bandwidth is available.
2. **SQLite contention.** Enabling WAL mode and batching study/instance
   writes would reduce lock contention under heavy inbound load.
3. **Unbounded associations.** An explicit association semaphore in the
   accept loop would prevent file-descriptor exhaustion from a runaway
   or hostile peer.

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
- **C-MOVE SCP / C-GET SCP**: Let workstations pull studies *from* Aurabox via
  Bounce. Bounce already acts as a C-FIND SCP and a C-STORE SCU/SCP; this would
  close the matrix and remove the dependency on the web viewer / share path for
  workstation-initiated retrieval.
- **On-the-fly transcoding for outbound C-STORE**: When a destination PACS does
  not accept any of the syntaxes Aurabox holds the study in, transcode to
  Explicit VR Little Endian before sending (currently the send fails with a
  clear error in that case).
- **Mid-send resume**: Reopen the association and resume from the next un-acked
  SOP instance UID after a transient TCP failure during a send.
- **Per-destination concurrency**: Cap concurrent C-STORE associations per
  destination PACS rather than per gateway.
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
