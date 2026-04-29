# Architecture Overview

This document describes the high-level architecture of Bounce, a DICOM C-STORE receiver application.

## Table of Contents

- [System Overview](#system-overview)
- [Component Architecture](#component-architecture)
- [DICOM Endpoint Model](#dicom-endpoint-model)
- [Aurabox Communication](#aurabox-communication)
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

## DICOM Endpoint Model

Bounce participates in DICOM communication in three distinct roles. Each role
has a different model for *which* endpoint(s) it talks to and *where the
endpoint configuration lives*. Understanding this split is essential before
making changes to networking, configuration, or job dispatch.

### Roles at a glance

| Role | Direction | Endpoints | Source of truth |
|------|-----------|-----------|-----------------|
| C-STORE SCP (receiver) | Inbound  | **Single** listening socket | Local `Config` (`store/config.rs`) |
| C-FIND SCU (query)     | Outbound | **Many** remote PACS        | Aurabox (per-job payload) |
| C-MOVE SCU (retrieve)  | Outbound | **Many** remote PACS        | Aurabox (per-job payload) |
| C-STORE SCU (send)     | Outbound | **Many** destination PACS   | Aurabox (per-job payload) |

### Inbound: single locally-configured listener

Bounce's receiver (`receiver/dicom_server.rs`) binds **one** TCP socket and
advertises **one** AE title. The listening parameters are stored locally and
edited via the Settings UI:

- `ip_address` — bind address (default `0.0.0.0`)
- `port`       — listening port (default `9090`; `104` is the DICOM well-known port)
- `ae_title`   — AE title advertised on associations (default `BOUNCE`)

These three fields live on the [`Config`](../src-tauri/src/store/config.rs)
struct and are persisted by the Tauri `plugin-store`. There is no list of
listeners — a Bounce instance is a single SCP. Multiple upstream
modalities/PACS may *connect to* this one listener concurrently, but Bounce
itself exposes one endpoint to the network.

Calling AE titles are **not** allow-listed in config; the SCP accepts
associations from any caller that can reach the socket. Access control is
delegated to the network layer (firewall, VPN, on-prem segmentation).

### Outbound: many endpoints, all supplied by Aurabox

For every outbound DIMSE role (C-FIND, C-MOVE, C-STORE-as-SCU), the remote
PACS connection details are **not** stored in Bounce's local config. Instead,
they arrive as part of each job payload fetched from Aurabox by
`query/poller.rs`:

- `PacsQueryRequest.service` — C-FIND target
  ([models.rs:30](../src-tauri/src/query/models.rs))
- `RetrieveJob.service` — C-MOVE source
  ([models.rs:343](../src-tauri/src/query/models.rs))
- `SendJob.destination` — C-STORE SCU target
  ([models.rs:352](../src-tauri/src/query/models.rs))

All three carry the same connection shape (`ae_title`, `host`, `port`) and
collapse into the lightweight [`PacsService`](../src-tauri/src/query/models.rs)
before being handed to the DIMSE execution code. This is why
`PacsService: From<DicomService> | From<RetrieveService> | From<SendDestination>`
is implemented three times — same wire-level shape, different job context.

```
┌──────────────────────────────────────────────────────────────────┐
│                          Aurabox                                  │
│  (source of truth for all remote PACS Bounce can talk to)         │
│                                                                   │
│   PACS A (host:port, AE)   PACS B (host:port, AE)   PACS C ...    │
└─────────────────────────┬─────────────────────────────────────────┘
                          │  GET /api/bounce/jobs/pending
                          │  (job payload contains connection details)
                          ▼
┌──────────────────────────────────────────────────────────────────┐
│                          Bounce                                   │
│                                                                   │
│   query/poller.rs ─► dispatch by job.type                         │
│        │                                                          │
│        ├─► RetrieveJob  → query/cmove.rs   (per-job target)       │
│        ├─► SendJob      → send/worker.rs   (per-job destination)  │
│        └─► PacsQuery…   → query/cfind.rs   (per-job service)      │
└──────────────────────────────────────────────────────────────────┘
```

#### Implications

- **No "remote PACS" entries exist in Bounce's settings.** Operators add,
  remove, or rename PACS in Aurabox; Bounce picks them up automatically on the
  next poll cycle. This keeps the on-prem deployment thin and means PACS
  topology can be changed without touching the desktop client.
- **Calling AE title for outbound associations.** When Bounce initiates an
  outbound association (C-FIND, C-MOVE, C-STORE SCU), it uses its own
  `ae_title` from local config as the *calling* AE — the same value it
  advertises on the SCP side. The *called* AE is taken from the job payload.
- **Job-scoped trust.** Bounce will dial whatever `host`/`port` arrives in a
  job. Authenticity is therefore anchored on the Aurabox API call (TLS +
  scoped API key). A compromised Aurabox tenancy could redirect Bounce to an
  arbitrary host; this is an accepted trust boundary for v1.
- **No local fallback.** If Aurabox is unreachable, Bounce cannot perform any
  outbound DIMSE operation, because it has no cached or operator-supplied
  PACS list. This is intentional: a single source of truth eliminates drift
  between cloud and on-prem service catalogs.

### What this means for changes

- Adding a new outbound DIMSE role: extend the unified jobs endpoint and the
  poller dispatch. Do **not** add PACS connection fields to `Config`.
- Adding multi-listener support (e.g., separate AE titles for separate
  tenancies): this would require extending `Config` from a single record to a
  collection and updating `receiver/dicom_server.rs` to bind multiple sockets.
  Currently out of scope.
- Local PACS allow-listing on the SCP: not currently supported; would need to
  be added to `Config` and enforced in `receiver/server.rs` at association
  acceptance.

---

## Aurabox Communication

Bounce does not receive its *operational* configuration from Aurabox — local
fields (`api_key`, `port`, `ae_title`, `ip_address`, `base_dir`,
`delete_after_success`, `send_logs`) live in the Tauri `plugin-store` and are
edited from the Settings UI. What Aurabox provides over HTTP is a small,
read-only set of **uploader config**, a **services catalog**, and **per-job
payloads**. There is no push channel; everything is HTTP polling or
on-demand fetches.

### Authentication and base URL

Both the credential and the routing destination collapse onto a single local
field, `api_key`. The key has the shape:

```
aura_<region>_bounce_<user>_<token>_<env>
```

- **Bearer token.** Every request sets `Authorization: Bearer {api_key}`
  (e.g. [`aura_api.rs:47`](../src-tauri/src/aura/aura_api.rs),
  [`query_api.rs:51`](../src-tauri/src/aura/query_api.rs)).
- **Base URL.** Derived by
  [`Config::get_api_endpoint()`](../src-tauri/src/store/config.rs).
  `region_from_api_key()` selects the host
  (`https://{region}.aurabox.app`); `mode_from_api_key()` overrides it for
  `staging`, `dev`, and `local` environments.

There is no separate "Aurabox URL" setting — rotating tenancies or
environments is done by issuing a new API key, not by reconfiguring Bounce.

### Endpoints consumed

| Endpoint | Method | Purpose | Caller |
|----------|--------|---------|--------|
| `/api/bounce/config` | GET | Uploader config: TUS `endpoint`, `token`, `bucket`, `mode`, `assembly_id`, plus a `lift` block | [`aura_api.rs::upload_config`](../src-tauri/src/aura/aura_api.rs) |
| `/api/bounce/queries/services` | GET | Available remote PACS catalog (`DicomService[]`) for UI display | [`query_api.rs::fetch_services`](../src-tauri/src/aura/query_api.rs) |
| `/api/bounce/queries/pending` | GET | Pending C-FIND queries (legacy) | [`query_api.rs::fetch_pending_queries`](../src-tauri/src/aura/query_api.rs) |
| `/api/bounce/jobs/pending` | GET | Unified jobs payload (retrieves + sends); each job carries its own PACS connection details | [`query_api.rs::fetch_pending_jobs`](../src-tauri/src/aura/query_api.rs) |
| `/api/bounce/find` | POST | Ad-hoc study search proxied to Aurabox | [`query_api.rs::find_studies`](../src-tauri/src/aura/query_api.rs) |
| `/api/bounce/queries/{id}/results` | POST | Report C-FIND results | `query_api.rs::post_query_results` |
| `/api/bounce/queries/{id}/failed` | POST | Report C-FIND failure | `query_api.rs::post_query_failed` |
| `/api/bounce/retrieves/{id}/completed` | POST | Report C-MOVE completion | `query_api.rs::post_retrieve_completed` |
| `/api/bounce/retrieves/{id}/failed` | POST | Report C-MOVE failure | `query_api.rs::post_retrieve_failed` |
| `/api/bounce/sends/{id}/progress` | POST | Stream send progress | `query_api.rs::post_send_progress` |
| `/api/bounce/sends/{id}/completed` | POST | Report C-STORE SCU completion | `query_api.rs::post_send_completed` |
| `/api/bounce/sends/{id}/failed` | POST | Report C-STORE SCU failure | `query_api.rs::post_send_failed` |
| `/api/bounce/upload/init` | POST | Begin a study upload (assembly handshake) | `aura_api.rs::upload_init` |
| `/api/bounce/upload/{path}` | POST | Complete a study upload | `aura_api.rs::upload_save` |

Of the GETs, only `/api/bounce/config` resembles "config from Aurabox" — it
carries the dynamic parts of the upload pipeline (TUS endpoint, bearer token,
S3 bucket). The other GETs return *work* (jobs, queries) or *catalog data*
(services), not settings.

### When fetches happen

```
                ┌─────────────────────────────────────────┐
                │  Startup (main.rs)                      │
                │   ├─ load Config from plugin-store      │
                │   └─ AuraApi::upload_config()           │
                │        (probe; surfaces "Could not      │
                │         reach …" if it fails)           │
                └─────────────────────────────────────────┘
                                  │
                                  ▼
                ┌─────────────────────────────────────────┐
                │  Background poller (query/poller.rs)    │
                │   loop every POLL_INTERVAL (5 s):       │
                │     ├─ skip if api_key is empty         │
                │     ├─ fetch_pending_queries()          │
                │     └─ fetch_pending_jobs()             │
                │          ├─ retrieves: sequential       │
                │          └─ sends: semaphore (max 2)    │
                └─────────────────────────────────────────┘
                                  │
                                  ▼
                ┌─────────────────────────────────────────┐
                │  On-demand                              │
                │   ├─ fetch_services()  (UI dropdowns)   │
                │   ├─ find_studies()    (study search)   │
                │   ├─ upload_config()   (per upload,     │
                │   │     fresh — no caching)             │
                │   └─ post_*           (job reporting)   │
                └─────────────────────────────────────────┘
```

Key cadence facts:

- Poll cadence is fixed at [`POLL_INTERVAL = 5 s`](../src-tauri/src/query/poller.rs);
  there is no exponential backoff, but the tick is skipped entirely when
  `api_key` is empty so an unconfigured install does not generate traffic.
- Outbound C-STORE sends are dispatched into a tokio task pool capped at
  [`MAX_CONCURRENT_SENDS = 2`](../src-tauri/src/query/poller.rs) per Bounce
  gateway. Retrieves run sequentially because they all share the local SCP
  receive pipeline.
- Uploader config is **not cached**. Every upload calls
  `aura_api.upload_config()` again, so Aurabox can rotate the TUS bearer or
  retarget the bucket without restarting Bounce — change takes effect on the
  next upload.

### Trust and offline behaviour

Authenticity is anchored entirely on TLS plus the local API key. Notable
consequences:

- **Receive path is independent of Aurabox.** The C-STORE SCP keeps
  accepting associations and queuing studies on disk even when Aurabox is
  unreachable. Backlogged studies upload when connectivity returns.
- **Outbound DIMSE requires Aurabox.** C-FIND, C-MOVE, and C-STORE SCU jobs
  all originate from the polled jobs feed. Bounce holds no offline cache of
  jobs or the services catalog, so a partition stops outbound work entirely.
- **Uploads require Aurabox.** Without a fresh `upload_config()` response
  Bounce cannot start a TUS upload; partial/in-flight uploads continue
  against the existing TUS endpoint and resume per the TUS protocol when
  reconnected.
- **Trust transitivity.** A compromised Aurabox tenancy could redirect
  Bounce to dial arbitrary `host`/`port`/`AE` values via the jobs feed. This
  is an accepted v1 trust boundary; mitigations (signed job payloads, local
  PACS allow-list) are out of scope.

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
