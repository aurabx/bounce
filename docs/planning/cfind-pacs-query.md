# C-FIND PACS Query via Aurabox

## Overview

Add the ability for Aurabox to request a DICOM C-FIND query against a PACS connected to a Bounce gateway. This enables users in the Aurabox web UI to search a remote PACS for studies without direct network access to it.

Bounce currently operates as a receive-only DICOM node (C-STORE SCP). This feature adds a DICOM SCU (client) capability, allowing Bounce to initiate outbound C-FIND requests to a configured PACS on behalf of Aurabox.

## Architecture

### Communication Model

Bounce sits inside a hospital/clinic network behind a firewall. Aurabox is in the cloud. Inbound connections to Bounce are blocked. All existing communication (uploads, config fetches) is outbound HTTP from Bounce to Aurabox.

The C-FIND feature maintains this pattern using **polling**: Bounce periodically asks Aurabox if there are pending queries, executes them locally against the PACS, and posts results back.

```
Aurabox Cloud                          Hospital Network
+------------------+                   +-----------------------------------+
|                  |                   |                                   |
|  User searches   |                   |  Bounce Gateway                   |
|  in Aurabox UI   |                   |  +-----------------------------+  |
|       |          |                   |  |                             |  |
|       v          |   (2) Poll        |  |  Poll loop (every 2-5s)    |  |
|  Create pending  |<--GET /pending----|  |                             |  |
|  PacsQuery row   |                   |  |  Receives pending query     |  |
|                  |---query params--->|  |       |                     |  |
|                  |                   |  |       v                     |  |
|                  |                   |  |  C-FIND SCU  ----DICOM----> |  PACS
|                  |                   |  |       |                     |  |
|                  |   (3) Results     |  |       v                     |  |
|  Store results,  |<--POST /results---|  |  Post results to Aurabox   |  |
|  mark complete   |                   |  |                             |  |
|       |          |                   |  +-----------------------------+  |
|       v          |                   |                                   |
|  Push to user    |                   +-----------------------------------+
|  via broadcast   |
|                  |
+------------------+
```

### Why Polling

The firewall constraint means Aurabox cannot push requests to Bounce. The options considered:

| Approach | Latency | Complexity | Laravel fit |
|----------|---------|------------|-------------|
| **Polling (chosen)** | 0-5s added | Low | Native REST, trivial |
| WebSocket | ~0s | Medium | Requires Reverb, non-standard client |
| SSE | ~0s | Medium | Keeps PHP process alive per gateway |
| Message queue | ~0s | High | New infrastructure dependency |

**Polling wins because:**
- It matches the existing outbound-only communication pattern
- Laravel handles it natively (two routes, a model, a controller)
- No firewall changes, no new infrastructure
- The C-FIND itself takes 1-3 seconds, so 2-5s of poll latency is imperceptible
- A 2-second poll interval adds negligible load (one small GET per 2 seconds)

### Latency Analysis

Worst-case user-perceived timeline with 5-second polling:

| Step | Duration | Cumulative |
|------|----------|------------|
| User clicks search in Aurabox | instant | 0s |
| Laravel creates PacsQuery record | ~50ms | ~0s |
| Bounce picks up query on next poll | 0-5s | 0-5s |
| DICOM association establishment | 200-500ms | 1-6s |
| C-FIND executes on PACS | 0.5-3s | 1-9s |
| Bounce posts results to Aurabox | 100-300ms | 2-9s |
| Results appear in Aurabox UI | ~100ms | 2-9s |

With a 2-second poll interval, the typical experience is 3-6 seconds. This feels normal for a PACS query.

**Adaptive polling** can tighten this further: poll at 10s when idle, 1-2s when the user is actively in the query UI.

## PACS Connection Configuration

### Service Model

PACS connection details (AE title, host, port) are configured in Aurabox as a **Service** entity associated with a Bounce gateway. This means:

- Aurabox knows which PACS a gateway can reach
- Multiple PACS can be configured per gateway
- The query request includes which Service (PACS) to query
- Bounce receives the PACS connection details as part of the query payload

### Bounce Config Additions

Bounce needs no static PACS config in its local settings. The PACS connection details come from Aurabox with each query request. Bounce does need its own AE title (already configured as `ae_title`) to identify itself as the calling SCU.

## Implementation Plan

### 1. Aurabox (Laravel) Changes

#### New Model: `PacsQuery`

| Column | Type | Description |
|--------|------|-------------|
| id | uuid | Primary key |
| gateway_id | uuid | FK to the Bounce gateway |
| service_id | uuid | FK to the PACS Service (has AE, host, port) |
| status | enum | `pending`, `processing`, `completed`, `failed` |
| query_level | string | `STUDY`, `SERIES`, or `IMAGE` |
| patient_name | string, nullable | Match filter |
| patient_id | string, nullable | Match filter |
| study_date | string, nullable | Match filter (DICOM date range) |
| accession_number | string, nullable | Match filter |
| modality | string, nullable | Match filter |
| results | json, nullable | C-FIND results array |
| error | text, nullable | Error message if failed |
| created_at | timestamp | |
| completed_at | timestamp, nullable | |

#### New API Endpoints

```
GET  /api/bounce/queries/pending
```
Returns pending queries for this gateway (identified by API key). Response includes PACS connection details from the associated Service.

```json
{
  "queries": [
    {
      "id": "uuid",
      "service": {
        "ae_title": "PACS_SCP",
        "host": "192.168.1.100",
        "port": 104
      },
      "query_level": "STUDY",
      "filters": {
        "patient_name": "DOE^JOHN",
        "study_date": "20240101-20241231",
        "modality": "CT"
      }
    }
  ]
}
```

```
POST /api/bounce/queries/{id}/results
```
Bounce posts C-FIND results back. Marks the query as `completed`.

```json
{
  "results": [
    {
      "patient_name": "DOE^JOHN",
      "patient_id": "12345",
      "study_date": "20240615",
      "study_time": "143022",
      "study_description": "CT CHEST W/CONTRAST",
      "accession_number": "ACC001",
      "study_instance_uid": "1.2.840...",
      "modalities_in_study": "CT",
      "number_of_series": 3,
      "number_of_instances": 245
    }
  ]
}
```

```
POST /api/bounce/queries/{id}/failed
```
Bounce reports a failure (PACS unreachable, association rejected, etc.).

```json
{
  "error": "Failed to establish DICOM association: connection refused"
}
```

### 2. Bounce (Rust) Backend Changes

#### New Module: `src-tauri/src/query/`

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports |
| `cfind.rs` | C-FIND SCU implementation |
| `poller.rs` | Background polling loop |
| `models.rs` | Query/result data structures |

#### C-FIND SCU (`query/cfind.rs`)

Uses `dicom_ul::association::client::ClientAssociationOptions` to establish an outbound DICOM association as an SCU, then sends a C-FIND-RQ and collects responses.

**Association setup:**
- Calling AE title: Bounce's configured `ae_title`
- Called AE title: from the Service config in the query payload
- Abstract syntax: `1.2.840.10008.5.1.4.1.2.2.1` (Study Root Query/Retrieve - FIND)
- Transfer syntaxes: Implicit VR LE + Explicit VR LE

**C-FIND-RQ command object:**
- `CommandField` = `0x0020` (C-FIND-RQ)
- `AffectedSOPClassUID` = Study Root Q/R FIND
- `CommandDataSetType` = `0x0000` (dataset present)
- `Priority` = `0x0000` (medium)

**C-FIND identifier (data object):**
- `QueryRetrieveLevel` = `"STUDY"` (required)
- Populated filter fields from the query (e.g. `PatientName`, `StudyDate`)
- Empty return-key fields (e.g. `StudyDescription`, `AccessionNumber`, `NumberOfStudyRelatedSeries`)

**Response handling loop:**
- Status `0xFF00` / `0xFF01` (Pending): parse data object, add to results
- Status `0x0000` (Success): done, no more results
- Other status: error

The `dicom-ul` crate (already a dependency with the `async` feature) provides `ClientAssociationOptions::establish()` (sync) or `establish_async()` (async via tokio). The existing codebase already demonstrates building `InMemDicomObject` command objects and parsing PData PDUs.

#### Query Poller (`query/poller.rs`)

A background `tokio::spawn` loop that:

1. Runs while the DICOM server is active
2. Every N seconds, calls `GET /api/bounce/queries/pending`
3. For each pending query:
   a. Calls C-FIND against the specified PACS
   b. Posts results to `POST /api/bounce/queries/{id}/results`
   c. On failure, posts to `POST /api/bounce/queries/{id}/failed`
4. Respects a shutdown signal (same pattern as the DICOM server's `tokio::select!`)

#### Data Models (`query/models.rs`)

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PacsQueryRequest {
    pub id: String,
    pub service: PacsService,
    pub query_level: String,
    pub filters: QueryFilters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PacsService {
    pub ae_title: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryFilters {
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub study_date: Option<String>,
    pub accession_number: Option<String>,
    pub modality: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CfindResult {
    pub patient_name: Option<String>,
    pub patient_id: Option<String>,
    pub study_date: Option<String>,
    pub study_time: Option<String>,
    pub study_description: Option<String>,
    pub accession_number: Option<String>,
    pub study_instance_uid: String,
    pub modalities_in_study: Option<String>,
    pub number_of_series: Option<u32>,
    pub number_of_instances: Option<u32>,
}
```

#### Aurabox API Additions (`aura/aura_api.rs`)

New methods on `AuraApi`:

- `fetch_pending_queries()` -> `Vec<PacsQueryRequest>`
- `post_query_results(query_id, results)` -> acknowledgement
- `post_query_failed(query_id, error)` -> acknowledgement

#### Main Setup (`main.rs`)

Start the poller in the Tauri `setup()` block, alongside the existing event listeners. The poller should start/stop with the DICOM server (or run independently if queries should work even when not receiving).

#### New Tauri Command (optional)

A `cfind_query` command could allow the Bounce UI to trigger a C-FIND directly for testing/diagnostics, independent of Aurabox:

```rust
#[tauri::command]
async fn cfind_query(
    app: AppHandle,
    pacs_host: String,
    pacs_port: u16,
    pacs_ae_title: String,
    patient_name: Option<String>,
    study_date: Option<String>,
) -> Result<Vec<CfindResult>, String> { ... }
```

### 3. Bounce Frontend Changes (Optional)

A diagnostics section on the Tools page to manually test C-FIND against a PACS. Not required for the Aurabox-driven flow but useful for setup and troubleshooting.

### 4. Cargo Dependencies

No new crate dependencies are required. The existing `dicom-ul` with the `async` feature provides `ClientAssociationOptions` and async `establish_async()` / `send()` / `receive()`. The existing `reqwest` handles the HTTP polling.

## DICOM C-FIND Protocol Reference

### SOP Class UIDs

| Level | SOP Class UID |
|-------|--------------|
| Patient Root - FIND | `1.2.840.10008.5.1.4.1.2.1.1` |
| Study Root - FIND | `1.2.840.10008.5.1.4.1.2.2.1` |

Study Root is the standard choice. Patient Root is used when the PACS organises by patient first.

### Command Field Values

| Command | Value |
|---------|-------|
| C-FIND-RQ | `0x0020` |
| C-FIND-RSP | `0x8020` |

### Status Codes in C-FIND-RSP

| Status | Meaning |
|--------|---------|
| `0x0000` | Success (final response, no dataset) |
| `0xFF00` | Pending (matching result follows) |
| `0xFF01` | Pending with warnings |
| `0xA700` | Refused: out of resources |
| `0xA900` | Identifier does not match SOP class |
| `0xC000`-`0xCFFF` | Unable to process |

### Study-Level Return Keys

These DICOM tags should be included in the C-FIND identifier. Empty values request the PACS to return them; populated values filter results.

| Tag | Keyword | Type |
|-----|---------|------|
| (0008,0020) | StudyDate | Filter/Return |
| (0008,0030) | StudyTime | Return |
| (0008,0050) | AccessionNumber | Filter/Return |
| (0008,0061) | ModalitiesInStudy | Filter/Return |
| (0008,1030) | StudyDescription | Return |
| (0010,0010) | PatientName | Filter/Return |
| (0010,0020) | PatientID | Filter/Return |
| (0010,0030) | PatientBirthDate | Return |
| (0010,0040) | PatientSex | Return |
| (0020,000D) | StudyInstanceUID | Return |
| (0020,0010) | StudyID | Return |
| (0020,1206) | NumberOfStudyRelatedSeries | Return |
| (0020,1208) | NumberOfStudyRelatedInstances | Return |

## Open Questions

1. **Max results** -- Should C-FIND responses be capped (e.g. 100 results) to avoid overwhelming the UI or network? Most PACS support a limit, but it's not standardised.
2. **Series/Image level queries** -- Start with study-level only, or support drill-down from the start?
3. **C-MOVE follow-up** -- Once results are shown, should Aurabox be able to trigger a C-MOVE/C-GET to retrieve the study via Bounce? This is the natural next step but a separate feature.
4. **Multiple PACS** -- A gateway may have access to more than one PACS. The Service model handles this, but the polling/routing logic needs to account for it.
5. **Query timeout** -- What's a reasonable timeout for C-FIND? 30 seconds is typical. Queries that hang should be marked as failed.
6. **Polling lifecycle** -- Should the poller run only while the DICOM server is active, or independently?
