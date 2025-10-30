# API Reference

This document provides a complete reference for Bounce's Tauri commands (backend API) and Aurabox REST API endpoints.

## Table of Contents

- [Tauri Commands](#tauri-commands)
- [Events](#events)
- [Aurabox REST API](#aurabox-rest-api)
- [Data Models](#data-models)
- [Error Handling](#error-handling)
- [Usage Examples](#usage-examples)

---

## Tauri Commands

Tauri commands are functions exposed from the Rust backend that can be called from the frontend via Tauri's IPC bridge.

### `receiver_start`

Start the DICOM C-STORE receiver server.

**Signature**:
```rust
async fn receiver_start(app: AppHandle) -> Result<(), String>
```

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('receiver_start');
  console.log('DICOM receiver started successfully');
} catch (error) {
  console.error('Failed to start receiver:', error);
}
```

**Behavior**:
- Reads configuration (port, IP address, AE title)
- Binds to configured TCP port
- Begins listening for DICOM associations
- Emits "log" event on success/failure
- Spawns async task for connection handling

**Possible Errors**:
- `"Failed to start server: Address already in use"` - Port is in use
- `"Failed to start server: Permission denied"` - Insufficient privileges for port <1024
- `"Failed to start server: ..."` - Other network errors

**Related Events**: `log`

---

### `receiver_stop`

Stop the DICOM C-STORE receiver server.

**Signature**:
```rust
async fn receiver_stop(app: AppHandle) -> Result<(), String>
```

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('receiver_stop');
  console.log('DICOM receiver stopped successfully');
} catch (error) {
  console.error('Failed to stop receiver:', error);
}
```

**Behavior**:
- Gracefully stops accepting new connections
- Allows existing connections to complete
- Emits "log" event on success/failure

**Possible Errors**:
- `"Failed to stop server: ..."` - Server stop error

**Related Events**: `log`

---

### `send_study`

Manually trigger upload of a specific study to Aurabox.

**Signature**:
```rust
async fn send_study(app: AppHandle, study_uid: String) -> Result<(), String>
```

**Parameters**:
- `study_uid` (string): The DICOM Study Instance UID

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

const studyUid = '1.2.840.113619.2.55.3.12345678.987';

try {
  await invoke('send_study', { study_uid: studyUid });
  console.log('Study upload initiated');
} catch (error) {
  console.error('Failed to send study:', error);
}
```

**Behavior**:
1. Compresses study folder to ZIP archive
2. Fetches upload configuration from Aurabox
3. Initializes upload session via Aurabox API
4. Uploads ZIP file using TUS protocol (chunked)
5. Marks upload as complete
6. Optionally deletes local files (based on config)
7. Updates study status in database

**Possible Errors**:
- `"send study panic"` - Generic upload error
- Network errors during upload
- Compression errors
- API authentication errors

**Related Events**: `log`, `upload-progress`

---

### `delete_study`

Delete a study from local storage and database.

**Signature**:
```rust
async fn delete_study(app: AppHandle, study_uid: String) -> Result<(), String>
```

**Parameters**:
- `study_uid` (string): The DICOM Study Instance UID

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

const studyUid = '1.2.840.113619.2.55.3.12345678.987';

try {
  await invoke('delete_study', { study_uid: studyUid });
  console.log('Study deleted successfully');
} catch (error) {
  console.error('Failed to delete study:', error);
}
```

**Behavior**:
1. Deletes study directory with all DICOM files
2. Deletes compressed ZIP archive (if exists)
3. Deletes metadata JSON file
4. Removes study record from database

**Warning**: This operation is irreversible. Ensure study is uploaded before deletion.

**Possible Errors**:
- `"delete study panic"` - Generic deletion error
- File system permission errors
- Database errors

---

### `reset_app`

Clear all studies and data from the application.

**Signature**:
```rust
async fn reset_app(app: AppHandle) -> Result<(), String>
```

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

if (confirm('Are you sure? This will delete all local data.')) {
  try {
    await invoke('reset_app');
    console.log('Application reset successfully');
  } catch (error) {
    console.error('Failed to reset app:', error);
  }
}
```

**Behavior**:
1. Truncates all database tables
2. Deletes all files in storage directory
3. Clears upload queue

**Warning**: This is a destructive operation. All local data will be lost.

**Possible Errors**:
- `"clear studies panic"` - Database clear error
- `"clear study panic"` - File deletion error

---

### `current_studies`

Fetch a paginated list of studies from the database.

**Signature**:
```rust
async fn current_studies(
    app: AppHandle, 
    page: Option<u32>, 
    limit: Option<u32>
) -> Result<(), String>
```

**Parameters**:
- `page` (number, optional): Page number (1-indexed), default: 1
- `limit` (number, optional): Items per page, default: 10

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Listen for the response
const unlisten = await listen('current-studies', (event) => {
  const studies = event.payload;
  console.log('Received studies:', studies);
  // Update UI with studies
});

// Request studies
try {
  await invoke('current_studies', { page: 1, limit: 20 });
} catch (error) {
  console.error('Failed to fetch studies:', error);
}

// Don't forget to cleanup
unlisten();
```

**Behavior**:
- Queries database for studies
- Returns paginated results
- Emits "current-studies" event with results

**Response Format**: See [StudyList](#studylist) model

---

### `api_start_upload`

Initialize an upload session with Aurabox (advanced usage).

**Signature**:
```rust
async fn api_start_upload(
    app: AppHandle,
    study_uid: String,
    signature: String,
    upload_id: String,
    assembly_id: String,
) -> Result<(), String>
```

**Parameters**:
- `study_uid` (string): Study Instance UID
- `signature` (string): Authentication signature
- `upload_id` (string): Unique upload identifier (UUID)
- `assembly_id` (string): Assembly identifier from Aurabox

**Note**: This is a low-level command typically used internally. Use `send_study` instead for normal uploads.

---

### `show_window`

Show and focus the main application window.

**Signature**:
```rust
fn show_window(app: AppHandle) -> Result<(), String>
```

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('show_window');
} catch (error) {
  console.error('Failed to show window:', error);
}
```

**Behavior**:
- Shows main window if hidden
- Brings window to front
- Sets focus to window

**Use Case**: Called from system tray menu to restore hidden window

---

### `send_log`

Send a log message to the backend logger.

**Signature**:
```rust
fn send_log(app: AppHandle, log: String) -> Result<(), String>
```

**Parameters**:
- `log` (string): Log message to record

**Frontend Usage**:
```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('send_log', { log: 'User clicked export button' });
} catch (error) {
  console.error('Failed to send log:', error);
}
```

**Behavior**:
- Prints log to console
- Emits "log" event to frontend
- Writes to log file
- Sends to remote logging (if enabled)

---

## Events

Events are emitted from the backend to the frontend for real-time updates.

### Event: `log`

**Payload**: `string`

Emitted when log messages are generated by the backend.

**Listen Example**:
```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<string>('log', (event) => {
  console.log('Backend log:', event.payload);
  // Append to log viewer in UI
});

// Cleanup when component unmounts
return () => { unlisten(); };
```

**Common Messages**:
- `"Starting server"`
- `"Stopping server"`
- `"New association from {AE_TITLE}"`
- `"Received StudyInstanceUID: {UID}"`
- `"Sending study {UID}"`
- `"Study data sent to aurabox {UID}"`

---

### Event: `current-studies`

**Payload**: `StudyList` (see [Data Models](#data-models))

Emitted in response to `current_studies` command.

**Listen Example**:
```typescript
import { listen } from '@tauri-apps/api/event';

interface Study {
  study_uid: string;
  patient_name?: string;
  patient_id?: string;
  study_date?: string;
  study_description?: string;
  modality?: string;
  status: string;
  created_at: string;
  updated_at: string;
}

const unlisten = await listen<Study[]>('current-studies', (event) => {
  const studies = event.payload;
  console.log(`Received ${studies.length} studies`);
  // Update UI state
  setStudies(studies);
});
```

---

### Event: `queue-study`

**Payload**: `{ study_uid: string }`

Emitted when a study is queued for upload (debounced).

**Listen Example**:
```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<{ study_uid: string }>('queue-study', (event) => {
  console.log('Study queued for upload:', event.payload.study_uid);
  // Show upload pending indicator
});
```

**Note**: This is an internal event. The actual upload happens 10 seconds after the last file is received.

---

### Event: `upload-progress`

**Payload**: `{ uploaded: number, total: number, progress: number }`

Emitted periodically during file upload to report progress.

**Listen Example**:
```typescript
import { listen } from '@tauri-apps/api/event';

interface UploadProgress {
  uploaded: number;  // Bytes uploaded so far
  total: number;     // Total bytes to upload
  progress: number;  // Percentage (0-100)
}

const unlisten = await listen<UploadProgress>('upload-progress', (event) => {
  const { uploaded, total, progress } = event.payload;
  console.log(`Upload progress: ${progress}% (${uploaded}/${total} bytes)`);
  // Update progress bar
  setUploadProgress(progress);
});
```

**Emission Frequency**: Every 50 MB uploaded or at completion

---

## Aurabox REST API

The Bounce backend communicates with Aurabox via REST API for upload coordination.

### Base URL

The API endpoint is determined from the API key:

```
https://{region}.aurabox.app
```

Where `{region}` is extracted from the API key (e.g., `au`, `us`, `eu`).

### Authentication

All requests include an `Authorization` header:

```
Authorization: Bearer {api_key}
```

---

### `GET /api/bounce/config`

Fetch upload configuration including TUS endpoint and credentials.

**Request**:
```http
GET /api/bounce/config HTTP/1.1
Host: au.aurabox.app
Authorization: Bearer aura_au_bounce_user_token_production
Content-Type: application/json
```

**Response** (200 OK):
```json
{
  "type": "lift",
  "mode": "bulk",
  "lift": {
    "endpoint": "https://lift.aurabox.cloud/files",
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "bucket": "dicom-uploads",
    "assembly_id": "asm_abc123def456"
  }
}
```

**Response Fields**:
- `type`: Upload type (always "lift")
- `mode`: Upload mode (always "bulk")
- `lift.endpoint`: TUS upload endpoint URL
- `lift.token`: Temporary authentication token for upload
- `lift.bucket`: Storage bucket name
- `lift.assembly_id`: Assembly identifier for this upload

**Error Responses**:
- `401 Unauthorized`: Invalid API key
- `403 Forbidden`: Account suspended or API key revoked
- `500 Internal Server Error`: Server error

---

### `POST /api/bounce/upload/init`

Initialize a new upload session.

**Request**:
```http
POST /api/bounce/upload/init HTTP/1.1
Host: au.aurabox.app
Authorization: Bearer aura_au_bounce_user_token_production
Content-Type: application/json

{
  "studies": [
    {
      "study_uid": "1.2.840.113619.2.55.3.12345678.987",
      "patient_name": "DOE^JOHN",
      "patient_id": "12345",
      "study_date": "20240115",
      "study_description": "CT CHEST W/CONTRAST",
      "modality": "CT",
      "series": [
        {
          "series_uid": "1.2.840.113619.2.55.3.12345678.987.1",
          "series_description": "Chest",
          "series_number": 1,
          "instances": 120
        }
      ]
    }
  ],
  "mode": "bulk",
  "type": "lift",
  "signature": "asm_abc123def456",
  "upload_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "upload_id": "550e8400-e29b-41d4-a716-446655440000",
  "message": "Upload initialized"
}
```

**Error Responses**:
- `400 Bad Request`: Invalid request body
- `401 Unauthorized`: Invalid API key
- `422 Unprocessable Entity`: Validation errors

---

### `POST /api/bounce/upload/start`

Signal that file upload has started.

**Request**:
```http
POST /api/bounce/upload/start HTTP/1.1
Host: au.aurabox.app
Authorization: Bearer aura_au_bounce_user_token_production
Content-Type: application/json

{
  "assembly_id": "asm_abc123def456",
  "type": "lift",
  "upload_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "message": "Upload started"
}
```

---

### `POST /api/bounce/upload/complete`

Mark upload as complete after file transfer finishes.

**Request**:
```http
POST /api/bounce/upload/complete HTTP/1.1
Host: au.aurabox.app
Authorization: Bearer aura_au_bounce_user_token_production
Content-Type: application/json

{
  "assembly_id": "asm_abc123def456",
  "type": "lift",
  "upload_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Response** (200 OK):
```json
{
  "success": true,
  "message": "Upload completed"
}
```

**Behavior**:
- Triggers post-processing on Aurabox backend
- Initiates DICOM parsing and indexing
- Notifies relevant users of new study

---

### `GET /api/bounce/signature`

Generate a signature for upload authentication (legacy/unused).

**Request**:
```http
GET /api/bounce/signature HTTP/1.1
Host: au.aurabox.app
Authorization: Bearer aura_au_bounce_user_token_production
```

**Response** (200 OK):
```json
{
  "signature": "asm_abc123def456",
  "expires_at": "2024-01-15T12:00:00Z"
}
```

**Note**: This endpoint is currently unused. Assembly IDs are provided in the upload config.

---

## Data Models

### Study

```typescript
interface Study {
  id?: number;                  // Database ID (auto-increment)
  study_uid: string;            // DICOM Study Instance UID
  patient_name?: string;        // Patient name (may be undefined)
  patient_id?: string;          // Patient ID (may be undefined)
  study_date?: string;          // Study date (YYYYMMDD format)
  study_description?: string;   // Study description
  modality?: string;            // Primary modality (CT, MR, US, etc.)
  status: string;               // "RECEIVED", "UPLOADING", "SENT", "ERROR"
  created_at: string;           // ISO 8601 timestamp
  updated_at: string;           // ISO 8601 timestamp
}
```

### StudyList

```typescript
type StudyList = Study[];
```

### Series

```typescript
interface Series {
  series_uid: string;           // DICOM Series Instance UID
  series_description?: string;  // Series description
  series_number?: number;       // Series number
  modality?: string;            // Modality for this series
  instances: number;            // Number of DICOM instances
}
```

### StudyMetadata

The metadata JSON file stored alongside studies:

```typescript
interface StudyMetadata {
  studies: Array<{
    study_uid: string;
    patient_name?: string;
    patient_id?: string;
    patient_birth_date?: string;
    patient_sex?: string;
    study_date?: string;
    study_time?: string;
    study_description?: string;
    accession_number?: string;
    referring_physician?: string;
    institution_name?: string;
    series: Array<{
      series_uid: string;
      series_description?: string;
      series_number?: number;
      modality?: string;
      body_part?: string;
      protocol_name?: string;
      instances: number;
    }>;
  }>;
  status: "RECEIVED" | "UPLOADING" | "SENT" | "ERROR";
  received_at: string;          // ISO 8601 timestamp
  uploaded_at?: string;         // ISO 8601 timestamp
}
```

### Config

```typescript
interface Config {
  api_key: string;              // Aurabox API key
  port: number;                 // DICOM receiver port (default: 9090)
  ip_address: string;           // Bind IP address (default: "0.0.0.0")
  ae_title: string;             // DICOM AE Title (default: "BOUNCE")
  base_dir: string;             // Storage directory path
  delete_after_success: string; // "yes" or "no"
  send_logs: string;            // "yes" or "no"
}
```

---

## Error Handling

### Frontend Error Handling

```typescript
import { invoke } from '@tauri-apps/api/core';

try {
  await invoke('receiver_start');
  // Success
} catch (error) {
  // Error is a string message
  if (error === 'Failed to start server: Address already in use') {
    alert('Port is already in use. Please choose a different port.');
  } else if (error === 'Failed to start server: Permission denied') {
    alert('Administrator privileges required for port 104.');
  } else {
    alert(`Error: ${error}`);
  }
}
```

### Backend Error Propagation

Rust functions return `Result<(), String>` where:
- `Ok(())` indicates success
- `Err(String)` contains error message

Example:
```rust
#[tauri::command]
async fn my_command() -> Result<(), String> {
    match some_operation().await {
        Ok(result) => Ok(()),
        Err(e) => Err(format!("Operation failed: {}", e))
    }
}
```

---

## Usage Examples

### Complete Upload Workflow

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// 1. Listen for events
const logUnlisten = await listen<string>('log', (event) => {
  console.log('Log:', event.payload);
  appendLog(event.payload);
});

const progressUnlisten = await listen<{ progress: number }>('upload-progress', (event) => {
  updateProgressBar(event.payload.progress);
});

// 2. Start DICOM receiver
try {
  await invoke('receiver_start');
  console.log('DICOM receiver started');
  setServerStatus('running');
} catch (error) {
  console.error('Failed to start:', error);
  showError(error);
}

// 3. Wait for studies to be received...
// (Automatic via C-STORE)

// 4. Fetch current studies
const studiesUnlisten = await listen<Study[]>('current-studies', (event) => {
  displayStudies(event.payload);
});

await invoke('current_studies', { page: 1, limit: 50 });

// 5. Manually trigger upload (if needed)
const studyUid = '1.2.840.113619.2.55.3.12345678.987';
try {
  await invoke('send_study', { study_uid: studyUid });
  console.log('Upload started');
} catch (error) {
  console.error('Upload failed:', error);
}

// 6. Cleanup listeners when done
logUnlisten();
progressUnlisten();
studiesUnlisten();
```

### Settings Management

```typescript
import { Store } from '@tauri-apps/plugin-store';

const store = new Store('store.json');

// Load settings
async function loadSettings() {
  const config = {
    api_key: await store.get<string>('api_key') || '',
    port: await store.get<string>('port') || '9090',
    ip_address: await store.get<string>('ip_address') || '0.0.0.0',
    ae_title: await store.get<string>('ae_title') || 'BOUNCE',
    base_dir: await store.get<string>('base_dir') || './tmp/dicom_storage',
    delete_after_success: await store.get<string>('delete_after_success') || 'no',
    send_logs: await store.get<string>('send_logs') || 'no',
  };
  return config;
}

// Save settings
async function saveSettings(config: Config) {
  await store.set('api_key', config.api_key);
  await store.set('port', config.port);
  await store.set('ip_address', config.ip_address);
  await store.set('ae_title', config.ae_title);
  await store.set('base_dir', config.base_dir);
  await store.set('delete_after_success', config.delete_after_success);
  await store.set('send_logs', config.send_logs);
  await store.save();
  console.log('Settings saved');
}
```

### Study Management

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Fetch and display studies
async function refreshStudies(page: number = 1, limit: number = 20) {
  const unlisten = await listen<Study[]>('current-studies', (event) => {
    const studies = event.payload;
    renderStudyTable(studies);
  });
  
  await invoke('current_studies', { page, limit });
  
  // Cleanup after a short delay
  setTimeout(() => unlisten(), 5000);
}

// Delete a study
async function deleteStudy(studyUid: string) {
  if (!confirm(`Delete study ${studyUid}?`)) return;
  
  try {
    await invoke('delete_study', { study_uid: studyUid });
    console.log('Study deleted');
    await refreshStudies(); // Refresh list
  } catch (error) {
    alert(`Failed to delete study: ${error}`);
  }
}

// Resend a study
async function resendStudy(studyUid: string) {
  try {
    await invoke('send_study', { study_uid: studyUid });
    console.log('Study resend initiated');
  } catch (error) {
    alert(`Failed to resend study: ${error}`);
  }
}
```

---

## Rate Limits

There are currently no enforced rate limits on the Aurabox API for Bounce clients. However, best practices recommend:

- Maximum 100 concurrent uploads per instance
- Chunk size: 5 MB for TUS uploads
- Retry failed requests with exponential backoff

---

## Versioning

The API follows semantic versioning. The current version is embedded in the API key and automatically negotiated.

**Future**: API version headers may be added:
```http
X-API-Version: 1.0
```

---

## Deprecation Policy

Deprecated endpoints will:
1. Be announced 6 months in advance
2. Continue to function for at least 12 months
3. Return `X-Deprecated: true` header
4. Include `Sunset` header with end-of-life date

---

## Support

For API-related questions:
- **Documentation**: https://docs.aurabox.cloud/api/
- **Support Email**: api-support@aurabox.cloud
- **Status Page**: https://status.aurabox.cloud
