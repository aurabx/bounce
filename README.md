# Bounce

<div align="center">

**Bounce** is a cross-platform desktop application that connects on-premises DICOM systems with the [Aurabox](https://aurabox.cloud) cloud platform as a full **bidirectional DIMSE gateway**.

[![License: BOUNCE EULA](https://img.shields.io/badge/License-Proprietary-red.svg)](LICENSE)
[![Platform: Windows | macOS | Linux](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue.svg)]()
[![Version](https://img.shields.io/badge/Version-1.8.0-green.svg)](CHANGELOG.md)

</div>

---

## Overview

Built with Tauri and Rust, Bounce runs behind a healthcare provider's firewall and brokers the four core DICOM services — **store, query, retrieve, and echo** — in both directions, so a local modality or PACS and the Aurabox cloud can each act as a peer of the other without exposing either directly.


### What Bounce Does

- **A local DICOM SCP** — Local systems (modalities, PACS, viewers) connect to Bounce as if it were a normal DICOM peer. Bounce handles their C-ECHO, C-STORE, C-FIND, and C-MOVE requests, and fulfils them against Aurabox over HTTPS.
- **A cloud-driven DICOM SCU** — Aurabox can drive Bounce to issue C-ECHO, C-FIND, C-MOVE, and C-STORE against any registered local PACS. A background poller pulls pending jobs from Aurabox, executes them as DIMSE operations on the local network, and reports progress, results, and failures back to the cloud.

### Bidirectional Flows

**From local systems into Aurabox**
- **Store** — Modalities and PACS push studies to Bounce via C-STORE; Bounce extracts metadata, aggregates instances into studies, compresses them, and forwards over the resumable TUS protocol with retry and crash-recovery.
- **Query** — Local viewers can issue C-FIND against Bounce to search Aurabox's catalogue without leaving the DICOM protocol.
- **Retrieve** — Local viewers can issue C-MOVE against Bounce to pull studies *out* of Aurabox; Bounce fetches them and stores them to the requesting destination AE.
- **Echo** — Standard C-ECHO connectivity verification.

**From Aurabox into local systems**
- **Query** — Aurabox queues C-FIND jobs against a registered local PACS; Bounce executes them as an SCU and returns the result set.
- **Retrieve** — Aurabox queues C-MOVE jobs; Bounce drives the remote PACS to send studies into Bounce's own C-STORE SCP, which then uploads them to Aurabox.
- **Send / push** — Aurabox can push a study to a local PACS: Bounce fetches the DICOM bytes from the Aurabox storage surface and issues C-STORE against the destination PACS as an SCU.
- **Echo** — Connectivity health checks against registered local PACS, on demand or on a schedule.

### Operational Capabilities

- **AE title allowlist** — Inbound associations are rejected with `CallingAETitleNotRecognized` unless the calling AE title is registered as a known PACS. Outbound SCU operations target only PACS configured in the same registry.
- **Resumable cloud transfer** — Uploads to Aurabox use TUS over HTTPS with automatic retry, exponential backoff, recovery on restart, and a disk-space safety check.
- **Concurrent send throttling** — Outbound C-STORE jobs are bounded so a single slow remote PACS cannot starve the rest of the gateway.
- **Transactions view** — A live, paginated, server-side searchable history of every upload attempt, with manual **Stop** and **Retry** controls on stuck studies.
- **Unattended operation** — Optional *Start on login* and *Start receiver on app start* settings keep the gateway online across reboots and manual restarts.
- **Automatic updates** — Optional auto-update mode applies downloaded updates inside a configurable restart window and resumes the receiver afterwards.
- **System tray** — Runs in the background. Closing the window hides Bounce to the tray on Windows and Linux; only an explicit quit terminates the process.
- **Local persistence** — SQLite tracks study status, transmission history, and job state; configurable retention controls how long received files remain on disk.
- **Defensive boundaries** — DICOM UIDs and transfer syntaxes are validated before use; unsupported syntaxes are rejected rather than panicking; malformed configuration falls back to safe defaults.
- **Structured logging** — Backend logging via the `tracing` crate, with optional remote log delivery. API keys, upload tokens, and PHI are kept out of plain-text log lines.

---

## Quick Start

### Installation

1. Visit the [Releases page](https://github.com/aurabx/bounce/releases).
2. Download the installer for your platform:
   - **Windows** — `.msi` or `.exe`
   - **macOS** — `.dmg`
   - **Linux** — `.deb`, `.AppImage`, or `.tar.gz`
3. Run the installer.

### Configuration

1. Launch Bounce.
2. Open **Settings** and configure:
   - **API Key** — Your Aurabox API key (required).
   - **AE Title** — Application Entity title for DICOM (default `BOUNCE`).
   - **Port** — DICOM receiver port (default `104`; binding below 1024 requires elevated privileges on most systems).
   - **IP Address** — Network interface to bind to (default `0.0.0.0`).
   - **Storage Location** — Directory for temporary DICOM file storage.
   - **Delete After Send** — Remove local files after a successful upload.
   - **Startup** — Optionally enable *Start on login* and *Start receiver on app start* for unattended operation.
3. Click **Start Receiver** to begin accepting associations.

Detailed end-user setup instructions live at <https://docs.aurabox.cloud/applications/bounce/>.

---

## Testing Against a Real DICOM Peer

Use [DCMTK](https://dicom.offis.de/dcmtk.php.en) to drive Bounce as an SCP, and [NightOwl](https://aurabox.cloud/nightowl) — Aurabox's developer-grade test PACS — to act as a remote SCP that Bounce can drive as an SCU.

**Driving Bounce as an SCP (local-to-cloud flows)**

```bash
# C-ECHO connectivity
make dicom-echo
# or: echoscu -aec BOUNCE 127.0.0.1 104

# C-STORE a file (must come from a registered AE title)
make dicom-send FILE=/path/to/test.dcm
# or: storescu -aec BOUNCE 127.0.0.1 104 /path/to/test.dcm

# C-FIND (study-level query against Aurabox via Bounce)
findscu -S -aec BOUNCE 127.0.0.1 104 -k 0008,0052="STUDY" -k 0010,0020=""

# C-MOVE (pull a study out of Aurabox to a local destination)
movescu -S -aec BOUNCE -aem LOCALVIEWER 127.0.0.1 104 -k 0008,0052="STUDY" -k 0020,000D=<StudyInstanceUID>
```

**Testing the SCU side (cloud-to-local flows)**

[NightOwl](https://aurabox.cloud/nightowl) is the recommended stand-in for an on-premises PACS that Bounce can query, retrieve from, and push to. Install it from <https://aurabox.cloud/nightowl> and run it locally, then register it as a known PACS in the Bounce **PACS** page using the AE title, host, and DIMSE port shown in NightOwl's configuration.

Once registered, Aurabox-driven query, retrieve, and send jobs targeting that PACS will be executed by Bounce against the local NightOwl instance — exercising the full cloud-to-local SCU path (C-ECHO, C-FIND, C-MOVE, and outbound C-STORE) end-to-end.

---

## Troubleshooting

**DICOM receiver will not start**
- Confirm the configured port is free and not blocked by the host firewall.
- Binding to port 104 requires elevated privileges on Linux and macOS. Use a port above 1024 to run as a normal user.
- Check the **Logs** page or the log files under `~/.aurabox/bounce/logs/`.

**Incoming associations rejected**
- Verify the sender's AE title is registered under **PACS** with the exact case it presents during negotiation.
- A rejection with `CallingAETitleNotRecognized` always indicates a missing or mismatched entry — DICOM UIDs and AE titles are case-sensitive.

**Files are not uploading**
- Confirm the API key is correct and the host can reach the Aurabox endpoint over HTTPS.
- Open the **Transactions** view to inspect upload-attempt history, error messages, and retry state.
- A study stuck in **RETRYING** can be cancelled with the **Stop** button; it will move to **FAILED** while preserving its attempt history.

**Application will not launch**
- Verify the OS meets the minimum requirements (Windows 10+, macOS 10.13+, modern Linux with WebKitGTK).
- Launch from a terminal to capture startup diagnostics.

---

## License

See [LICENSE](LICENSE) (BOUNCE EULA — proprietary).

---

## Support

- **Email** — support@aurabox.cloud
- **Documentation** — <https://docs.aurabox.cloud>
- **Bug reports** — <https://github.com/aurabx/bounce/issues>