# Bounce 2.0 — Headless, Multi-Profile, Multi-Worker Refactor

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`
must be kept up to date as work proceeds. This plan does not depend on any
prior plan; everything required to implement it is included here.

There is no `PLANS.md` checked into the repository at the time this plan
was written. Follow the conventions in
`.claude/skills/codex-plans/SKILL.md` (the codex-plans skill bundled with
the project) when revising this document.

## Purpose / Big Picture

Today, Bounce is a Tauri desktop application. It binds one DICOM listener,
runs a single-flight uploader, and reads all of its operational
configuration from a Tauri-managed JSON store edited through the
Next.js UI. There is no way to run it without a graphical session, and
there is no way to have one machine accept DICOM traffic on behalf of
more than one logical "tenant" or upload destination.

After this refactor, an operator can take a single `bounce.json` file,
copy it onto a Linux box with no display server, and run:

    bounced --config /etc/bounce/bounce.json

The resulting process will:

1. Bind **multiple DICOM SCP listeners simultaneously**. Each listener
   is described by a "profile" entry in the JSON config and has its own
   AE title and TCP port. All listeners may share the same bind IP
   (e.g. `0.0.0.0`). Every listener performs the full set of roles
   Bounce performs today: C-STORE SCP, C-ECHO SCP, C-FIND SCP (proxying
   to Aurabox), and C-MOVE SCP (fetching study bytes from Uhura via
   WADO-RS and forwarding via C-STORE SCU).
2. For each profile, run **N concurrent upload workers** (configurable
   per profile, default 2) that pull completed studies off a
   per-profile queue and ship them to Aurabox via TUS. Each profile
   also gets its own **Aurabox job-feed poller** that fetches pending
   queries, retrieves, sends, and echoes and reports results back —
   the same loop that lives in `query/poller.rs` today, but scoped to
   one profile (one tenancy, one API key) instead of being global.
3. Every profile in Bounce 2.0 talks to Aurabox. The product remains
   Aurabox-only; this plan does not add support for non-Aurabox
   targets. Operators who need multiple Aurabox tenancies on one host
   (e.g. one tenancy per clinic) configure multiple profiles, each
   with its own `api_key` and its own local AE/port. The point of
   profiles is multi-tenancy on one Bounce instance, not vendor
   neutrality.
4. Reusability for a hypothetical future product ("Bounce for X") is
   delivered through **crate boundaries**, not through runtime
   polymorphism inside Bounce. The DICOM-SCP framework, the
   per-profile worker pool, the TUS chunked uploader, and the JSON
   config loader live in standalone crates with no Aurabox knowledge.
   A future app would depend on those crates and write its own
   product-specific glue (its own equivalent of `aurabox-client`, its
   own config schema, its own CLI). Inside Bounce itself, the Aurabox
   glue is hardcoded and direct — no `dyn Backend` trait, no
   config-driven backend selection, no second built-in target to
   maintain.
5. Continue to expose the existing Tauri UI as an optional front-end
   that drives the same core runtime, so existing desktop installs
   keep working. The UI is no longer the source of truth for
   configuration; it reads and writes the same `bounce.json` schema.

A reader can confirm the new behaviour end-to-end by:

- Writing a `bounce.json` with two profiles bound to different ports
  (e.g. `11112` and `11113`), each pointing at its own Aurabox API
  key (which in test can be a `mockito`-backed fake Aurabox).
- Running `bounced --config ./bounce.json` and watching it log
  "listening on 0.0.0.0:11112 (profile=clinic-a)" and
  "listening on 0.0.0.0:11113 (profile=clinic-b)".
- Using `storescu` (from DCMTK) to send the same DICOM file to each
  port and seeing the file land in two distinct Aurabox tenancies
  (verified by the per-profile API key recorded against each upload
  in the fake Aurabox).
- Pointing `storescu` at one port with two concurrent invocations and
  seeing both uploads progress in parallel (worker count > 1) in the
  log output.
- Killing the host's network briefly during an upload, restoring it,
  and observing the existing TUS resumption behaviour still works
  per profile.

## Definitions

This plan uses a small set of terms that are *not* ordinary English in
this codebase. They are defined here so the rest of the plan stays
concrete.

- **Profile** — A named bundle of configuration in `bounce.json` that
  describes (a) one DICOM SCP socket Bounce should bind for a single
  Aurabox tenancy (b) the Aurabox API key and resulting upload target
  that studies received on that socket should be shipped to, and (c)
  the number of upload workers dedicated to it. Two profiles with the
  same `local.ip` but different `local.port` values produce two
  independent listeners on the same host. Every profile is Aurabox;
  Bounce 2.0 has no other backend.
- **Worker pool** — A bounded set of tokio tasks dedicated to one
  profile. Each task loops, awaits the next study UID from an
  `mpsc::Receiver`, and runs the full compress → upload → cleanup
  pipeline before requesting the next one.
- **Headless runtime** — The non-Tauri entry point. A binary named
  `bounced` that initialises logging, loads `bounce.json`, opens the
  SQLite database, constructs one `ProfileRuntime` per profile, and
  blocks on `tokio::signal::ctrl_c` for shutdown.
- **Reusable primitive crate** — A library crate produced by this plan
  that contains no Aurabox-specific code and could be depended on by a
  *different* product's "Bounce for X" gateway without modification.
  This plan produces three such crates: `dimse-gateway` (DICOM SCP
  framework with pluggable C-STORE / C-FIND / C-MOVE handler
  callbacks), `study-pipeline` (per-profile debounce + bounded MPSC +
  N-worker pool), and `tus-uploader` (chunked TUS POST/PATCH client
  taking endpoint, token, chunk size, and metadata as inputs). They
  are not used polymorphically inside Bounce; they are the layered
  building blocks Bounce composes against.
- **Product crate** — A library crate that wires reusable primitives
  to one specific cloud product. For Bounce 2.0 the product crate is
  `bounce-aurabox`, which depends on the three primitive crates plus
  an `aurabox-client` HTTP crate, and exposes a `Runtime` and
  `ProfileRuntime` shaped specifically for Aurabox semantics
  (`api_key`, the upload_config/init/save handshake, the poll loop
  against `/api/bounce/jobs/pending`). A future "Bounce for X" would
  add a sibling `bounce-x` product crate without touching the
  primitives.
- **App crate** — A binary crate. This plan produces two: `bounce-cli`
  (the headless `bounced` binary) and `bounce-tauri` (the desktop UI).
  Both depend on `bounce-aurabox`. A hypothetical "X for desktop"
  would be a third app crate depending on `bounce-x`.

## Progress

Every stopping point must be reflected here, even if a single task has
to be split into "done" and "remaining" halves. Use UTC timestamps when
recording completion.

- [ ] M0: Plan accepted; clarifying questions resolved or documented.
- [ ] M1: Workspace skeleton in place — crates
  `dimse-gateway`, `study-pipeline`, `tus-uploader`,
  `aurabox-client`, `bounce-aurabox`, `bounce-cli`, `bounce-tauri`
  all compile (initially as straight relocations of existing code,
  with no functional change).
- [ ] M2: `bounce-aurabox` no longer depends on `tauri`; all
  Tauri-only callouts (event emission, app-data-dir resolution,
  plugin-store config loading) are replaced with explicit
  constructor parameters and a small `EventSink` callback trait
  defined in `bounce-aurabox` itself.
- [ ] M3: New `bounce.json` schema, loader, and validator implemented
  in `bounce-aurabox::config`; round-trip tested with a minimal
  sample config.
- [ ] M4: `Profile` and `ProfileRuntime` types implemented in
  `bounce-aurabox`; the receiver binds per-profile listeners using
  `dimse-gateway` primitives; database carries a `profile_id`
  column; existing single-profile behaviour reproduced via a
  one-profile JSON file.
- [ ] M5: Per-profile bounded MPSC queue and N-worker pool from
  `study-pipeline` replace the single-flight debounced uploader;
  load test with two concurrent uploads shows both progressing.
- [ ] M6: Generic `tus-uploader` crate extracted, exposing a
  `TusClient` that takes endpoint/token/chunk-size/metadata as
  inputs. The existing Aurabox-specific `upload_init` /
  `upload_save` handshake stays in `bounce-aurabox` and calls into
  `TusClient` for the actual chunked upload.
- [ ] M7: `bounced` headless binary wired up; `bounced --config
  ./bounce.json` accepts DICOM on configured ports without a display
  server and ships to Aurabox. End-to-end test passes against the
  Orthanc test PACS and a mockito-backed fake Aurabox.
- [ ] M8: Tauri UI ported onto the new product crate; settings panel
  reads and writes `bounce.json`; existing desktop install upgrades
  cleanly.
- [ ] M9: A "vendor-neutrality probe" — a 50-line example binary in
  `dimse-gateway/examples/echo_only.rs` that uses *only* the
  primitive crates (no `bounce-aurabox`, no `aurabox-client`) to
  stand up a C-ECHO-only listener — compiles and runs. This is the
  acceptance test for "the primitives are actually reusable."
- [ ] M10: Documentation updated (README, ARCHITECTURE.md,
  CONFIGURATION.md, API.md); CHANGELOG entry added; version bumped
  to 2.0.0 via `make version V=2.0.0`.

## Surprises & Discoveries

Record unexpected behaviours, useful library quirks, and short evidence.
Empty at plan creation.

- (none yet)

## Decision Log

Decisions are recorded in the format below. The earliest entries
capture choices made while authoring the plan itself; later entries
must be appended as the work proceeds.

- Decision: Reusability for a hypothetical future "Bounce for X" is
  delivered via **crate boundaries**, not via runtime polymorphism
  inside Bounce. Bounce 2.0 stays Aurabox-only; there is no
  `dyn Backend` trait, no `match backend { ... }`, no JSON tag like
  `"type": "aurabox"`. The Aurabox glue is hardcoded and direct. The
  generic, vendor-neutral pieces are pulled into standalone crates
  (`dimse-gateway`, `study-pipeline`, `tus-uploader`) that a future
  product can depend on without inheriting any Aurabox-specific code.
  Rationale: The user explicitly said "Bounce will stay Aurabox only,
  but we want sufficient reusable components that we can build a
  similar app for a different product." Putting a runtime
  abstraction inside Bounce would force every Bounce contributor to
  reason about a multi-backend code path that has no second
  implementation, while still failing to give "Bounce for X" what it
  actually needs (which is its own config schema, its own CLI, its
  own HTTP client, its own UI). Crate-level reuse keeps Bounce
  itself simple and gives the future product real building blocks
  rather than a leaky abstraction.
  Date/Author: 2026-05-26 / planner.

- Decision: Split the existing single Cargo crate into a multi-crate
  workspace structured as **primitives → product → app**:
  - Primitive crates: `dimse-gateway` (DICOM SCP framework with
    pluggable C-STORE / C-FIND / C-MOVE handler callbacks),
    `study-pipeline` (debounce + bounded MPSC + N-worker pool),
    `tus-uploader` (chunked TUS POST/PATCH client).
  - Product crates: `aurabox-client` (HTTP client for the Aurabox
    REST surface), `bounce-aurabox` (the runtime that wires
    primitives + `aurabox-client` together; owns `BounceConfig`,
    `Runtime`, `ProfileRuntime`, the SQLite layer, the poller).
  - App crates: `bounce-cli` (headless `bounced` binary),
    `bounce-tauri` (desktop UI).
  Rationale: Tauri's build script and dependency tree are awkward
  to gate behind a Cargo feature, so a workspace is going to happen
  regardless. Once committed to a workspace, the cheap thing is to
  draw the boundary where reuse actually lives — between vendor-
  neutral DICOM/TUS/pipeline primitives and Aurabox-specific glue —
  rather than just between "library" and "binary". A future product
  swaps `aurabox-client` and `bounce-aurabox` for its own pair and
  keeps the primitives untouched.
  Date/Author: 2026-05-26 / planner.

- Decision: Each primitive crate must have at least one example
  binary or integration test that uses *only* primitive crates and
  the Rust standard library (plus `tokio` and `anyhow`). This is the
  acceptance test for "the primitive is actually reusable" and is
  what M9 in this plan validates. If during M2–M6 a primitive ends
  up reaching for Aurabox-shaped types, the smell is real and the
  primitive needs to be reshaped before we move on.
  Rationale: It is easy to claim a crate is reusable; it is much
  harder to claim it when there exists a build target that proves
  it can be used in isolation. The cost of one tiny example per
  primitive is trivial against the cost of discovering the boundary
  was wrong only when "Bounce for X" begins.
  Date/Author: 2026-05-26 / planner.

- Decision: The Tauri UI is *retained* in 2.0, not retired. It becomes
  one of two front-ends to the same `bounce-core`.
  Rationale: The user statement says 2.0 "needs to" run headless and
  describes profile semantics; it does not say the desktop UI is
  going away. Keeping the UI working preserves the existing install
  base and avoids a coupled UI rewrite landing inside an already large
  backend refactor. If the user later confirms the UI should be
  dropped, removing `bounce-tauri` is a single-crate deletion.
  Date/Author: 2026-05-26 / planner.

- Decision: One shared SQLite database file (`bounce.db`) for all
  profiles. The `studies` and `uploads` tables gain a `profile_id`
  text column. Queries that previously had no scope are now scoped
  by `profile_id`.
  Rationale: Per-profile databases would multiply file-handle usage
  and complicate the UI's "list everything I've ever received" view.
  A single database with a `profile_id` discriminator is the smallest
  change and matches how the existing migrations layer is structured.
  Date/Author: 2026-05-26 / planner.

- Decision: The JSON config is the single source of truth at runtime;
  the Tauri plugin-store is removed from `bounce-core`. The UI
  edit-and-save flow becomes "load `bounce.json` → mutate in memory →
  write `bounce.json` atomically → signal the running runtime to
  reload."
  Rationale: Two parallel config stores (plugin-store + JSON) would
  drift. Making JSON authoritative also lets ops teams put the file
  under config management (Ansible, etcd-rendered drops, etc.) without
  having to touch a plugin-store SQLite blob hidden in an app-data
  directory.
  Date/Author: 2026-05-26 / planner.

- Decision: The TUS chunk size (5 MB), the study debounce (10 s), and
  the worker count are *per-profile* configurable knobs, with defaults
  chosen to match today's behaviour. The receiver still has no global
  association cap in 2.0; per-profile association caps may be added
  later if motivated by an incident, and are explicitly out of scope
  here.
  Rationale: The user asked specifically for "multiple workers" and
  "profiles with a number of workers assigned." Surfacing the other
  knobs costs nothing and unblocks tuning per deployment without code
  changes. Adding association caps without a concrete need would
  expand scope.
  Date/Author: 2026-05-26 / planner.

- Decision: The outbound DIMSE poller (C-FIND, C-MOVE, C-STORE SCU,
  C-ECHO jobs from Aurabox) is scoped to a profile. Each profile
  gets its own poller, which uses that profile's `api_key` to call
  the Aurabox endpoints. The current single `MAX_CONCURRENT_SENDS`
  semaphore in `query/poller.rs` becomes a per-profile field
  (default 2) so a busy clinic does not starve another one running
  on the same host.
  Rationale: The poller is intrinsically Aurabox-shaped (it speaks
  the `/api/bounce/jobs/pending`, `/api/bounce/queries/pending`,
  and result-reporting endpoints). It naturally lives in
  `bounce-aurabox` and there is no reason to share state across
  profiles — each profile is a separate tenancy with separate work.
  Date/Author: 2026-05-26 / planner.

- Decision: The receiver's C-FIND SCP and C-MOVE SCP handlers stay
  Aurabox-aware inside `bounce-aurabox`. The `dimse-gateway`
  primitive crate exposes the SCP framework as a set of trait-based
  callback handlers — `CStoreHandler`, `CFindHandler`, `CMoveHandler`,
  `CEchoHandler` — and `bounce-aurabox` provides concrete handlers
  that close over the relevant `AuraboxClient` and `ProfileRuntime`.
  A future product implements its own handlers without `dimse-gateway`
  ever having to know what an upstream HTTP query looks like.
  Rationale: This is the natural seam between "DICOM-level wire
  protocol code" (generic) and "what to do with a received study or
  proxied query" (product-specific). It also matches how the existing
  `cfind_handler.rs` and `cmove_handler.rs` are already shaped — they
  accept an `&association` and an `&QueryApiClient`, which is exactly
  the trait-callback pattern in disguise.
  Date/Author: 2026-05-26 / planner.

## Outcomes & Retrospective

To be filled in at the end of each milestone and again at plan
completion. Compare actuals against the "Purpose / Big Picture"
section.

- (none yet)

## Context and Orientation

This section describes the relevant current state of the code so a
reader who has never seen this repo can follow the rest of the plan.

The repository root is `/Users/xtfer/working/aurabx/_active/bounce`.
The Rust backend lives under `src-tauri/src`. The Next.js frontend
lives under `app`. Build orchestration lives in `Makefile`,
`package.json`, and `src-tauri/Cargo.toml`. The `docs/` directory
contains `ARCHITECTURE.md`, `API.md`, `CONFIGURATION.md`,
`DEVELOPMENT.md`, and this plan.

The current backend is a single Cargo crate, `app`, declared in
`src-tauri/Cargo.toml`. Its `main.rs` builds a `tauri::Builder`,
registers IPC commands, and wires up background tasks. The relevant
modules are:

- `src-tauri/src/store/config.rs` — `Config` struct loaded synchronously
  from the Tauri plugin-store. Holds `api_key`, `port`, `ip_address`,
  `ae_title`, `base_dir`, `delete_after_success`, `send_logs`. Singular
  fields — there is no concept of more than one of any of them.
- `src-tauri/src/receiver/dicom_server.rs` — `DICOMServer::start()`
  binds a single `tokio::net::TcpListener` to `(config.ip_address,
  config.port)`, then loops on `listener.accept()` and processes each
  association serially inside `run_store_sync()`. Holds an
  `Option<AppHandle>` that it uses to emit `queue-study` and `log`
  events to the UI.
- `src-tauri/src/receiver/server.rs` — `start()` and `stop()` manage
  the receiver's lifecycle. State (the oneshot shutdown sender, the
  poller's shutdown sender) is stashed in Tauri-managed
  `Arc<Mutex<ServerState>>`.
- `src-tauri/src/transmitter/transmission.rs` — `Transmission` owns a
  `reqwest::Client`, a `HashMap` of debounce timers
  (`scheduled_studies`), and an `AppHandle`. `schedule_study_push()`
  runs the 10-second debounce, `send_study()` runs the full pipeline:
  `compress_study()` zips the study directory, `fetch_uploader_config()`
  calls Aurabox to get TUS endpoint + token + bucket, `upload_init()`
  hands metadata to Aurabox, `upload_via_tus()` does the chunked
  upload, `upload_save()` reports completion.
- `src-tauri/src/aura/aura_api.rs` and `aura/query_api.rs` — Aurabox
  HTTP clients. Construct a `reqwest::Client` per instance and read
  `api_key`/`api_endpoint` from `Config` at call time.
- `src-tauri/src/query/poller.rs` — Background loop that polls
  `/api/bounce/queries/pending` and `/api/bounce/jobs/pending` every
  5 seconds and dispatches retrieve/send/echo jobs. Sends are capped
  at `MAX_CONCURRENT_SENDS = 2` via a `tokio::sync::Semaphore`.
- `src-tauri/src/db/database.rs` — `Database` wraps a `SqlitePool`
  rooted at `app_data_dir/bounce.db`. Uses `app_handle.path()` to
  resolve the directory, so it cannot construct itself without a Tauri
  `AppHandle`.
- `src-tauri/src/logger.rs` — Custom `log::Log` implementation that
  batches log lines and POSTs them to Better Stack Logtail. Wired up
  in `main.rs::setup_logger_with_config()`.

The frontend (`app/`) reads and writes the plugin-store via Tauri IPC
and listens for the same events (`log`, `running`, `running-details`,
`current-studies`, `upload-progress`, `queue-study`). It does not
currently know anything about profiles.

Key coupling points between "Tauri" and "business logic" today:

1. **Config loading.** `Config::load(AppHandle)` reads from
   `app.store("store.json")`. Everywhere a piece of code needs config,
   it calls `crate::load_config(app)` which forwards to that.
2. **Event emission.** Server status, log messages, study progress are
   all delivered via `app_handle.emit("log", ...)`. Without an
   `AppHandle`, there is no place for the events to go.
3. **Database location.** `Database::new(AppHandle)` resolves the path
   via `app_handle.path().app_data_dir()`.
4. **Logger backend.** `tauri-plugin-log` is registered on the Tauri
   builder; in a headless world it would not exist.

The refactor's job is to make those four coupling points pluggable and
to introduce profile-aware container types around the existing logic.
The existing DICOM, TUS, and SQLite code is largely correct and can be
re-used in place once it stops reaching for `AppHandle`.

## Plan of Work

This section describes, in prose, the sequence of edits and additions.
Concrete shell commands and expected output appear in "Concrete Steps"
below.

### Phase 1: Workspace skeleton (M1)

Convert `src-tauri/` from a single crate into a Cargo workspace. The
top-level `src-tauri/Cargo.toml` becomes a virtual manifest with
seven members organised in three layers:

Primitive crates (vendor-neutral; no Aurabox or Tauri knowledge):

- `crates/dimse-gateway` — DICOM SCP framework. Wraps the
  `tokio::net::TcpListener` accept loop and the `dicom_ul` PDU
  parsing currently in `receiver/dicom_server.rs`. Exposes a
  `GatewayBuilder` that takes the bind address, AE title,
  abstract-syntax list, and a set of trait-shaped handlers
  (`CStoreHandler`, `CFindHandler`, `CMoveHandler`, `CEchoHandler`).
- `crates/study-pipeline` — Per-profile worker pool. The 10-second
  debounce, the bounded `mpsc` queue, the N upload-worker tasks.
  Takes a `Shipper` callback (`async fn ship(study_uid, &Path)
  -> Result<()>`) and a study directory provider.
- `crates/tus-uploader` — Chunked TUS client. Exposes a
  `TusClient::new(endpoint, token, chunk_size_mb,
  Vec<(metadata_key, metadata_value)>)` and an
  `async fn upload(&self, path: &Path, progress: impl Fn) -> Result<()>`.
  Internally mirrors the existing `upload_via_tus` code.

Product crates (Aurabox-specific):

- `crates/aurabox-client` — HTTP client. Exposes `AuraboxClient` with
  one method per existing Aurabox endpoint
  (`upload_config`, `upload_init`, `upload_save`, `fetch_services`,
  `fetch_pending_jobs`, `fetch_pending_queries`, `find_studies`,
  `post_query_results`, `post_send_completed`, …). Owns a
  `reqwest::Client` and an API key. Reusable inside Bounce, not
  inside "Bounce for X".
- `crates/bounce-aurabox` — Runtime. Defines `BounceConfig`,
  `ProfileConfig`, `Runtime`, `ProfileRuntime`, the SQLite layer,
  the per-profile poller, and the concrete `CStoreHandler` /
  `CFindHandler` / `CMoveHandler` implementations that compose the
  `dimse-gateway` framework with the `aurabox-client` HTTP calls.
  This is where Aurabox-specific logic lives.

App crates (binaries):

- `crates/bounce-cli` — Binary producing the `bounced` executable.
  Depends on `bounce-aurabox`. Implements `fn main()`, CLI argument
  parsing with `clap`, JSON config loading, runtime bring-up, and
  `tokio::signal::ctrl_c` shutdown handling. No Tauri.
- `crates/bounce-tauri` — Binary producing the existing
  `Aurabox Bounce` desktop bundle. Depends on `bounce-aurabox`.
  Contains the existing `main.rs` shrunk to Tauri glue: builder
  setup, plugin registration, IPC commands that delegate to
  `bounce_aurabox::Runtime`, tray icon code.

The first pass of M1 is mechanical: files relocate, paths in `mod`
statements update, `use crate::...` paths change to
`use bounce_aurabox::...` (or the appropriate primitive crate's
path) for cross-crate references. No behaviour changes.

### Phase 2: Break the Tauri dependency from `bounce-aurabox` (M2)

`bounce-aurabox` and every primitive crate must build without `tauri`
in their dependency tree. The current coupling points are addressed
as follows.

- `Config::load(AppHandle)` (plugin-store reads) is replaced by
  `BounceConfig::load_file(&Path)`. The Tauri UI gets a thin
  `TauriConfigSource` adapter in `bounce-tauri` that reads
  `bounce.json` via the OS filesystem rather than the plugin-store;
  this preserves the UI's ability to read and save settings without
  introducing a second source of truth.
- `app_handle.emit("topic", payload)` is replaced by an `EventSink`
  callback trait defined in `bounce-aurabox`:

        pub trait EventSink: Send + Sync + 'static {
            fn emit(&self, topic: &str, payload: serde_json::Value);
        }

  `bounce-cli` supplies a `LoggingEventSink` that turns every event
  into a structured log line. `bounce-tauri` supplies a
  `TauriEventSink` that bridges to `app.emit`. The receiver, the
  workers, and the poller all hold an `Arc<dyn EventSink>` and call
  it instead of touching `AppHandle`.
- `app_handle.path().app_data_dir()` is replaced by an explicit
  `data_dir: PathBuf` passed to `Runtime::start`. The CLI resolves
  it from the JSON config's `storage_dir` field; the Tauri binary
  resolves it via Tauri's own `app.path()`.
- `tauri-plugin-log` is replaced by `tracing` +
  `tracing-subscriber` initialised inside `bounce-aurabox`. Logs go
  to stdout by default, with a file-based subscriber added when the
  headless binary is given `--log-file PATH`. The existing Better
  Stack Logtail integration in `logger.rs` is preserved but moves
  into a `bounce-aurabox::telemetry::logtail` module, opt-in via the
  config's `telemetry` block.

### Phase 3: JSON config schema (M3)

Define a versioned config schema in `bounce-aurabox::config`. The
schema is shaped specifically for Aurabox — there is no `type:` tag
on the upload block, no second variant. A future "Bounce for X"
defines its own incompatible schema; the two are not expected to
share a JSON file.

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BounceConfig {
        /// Schema version. Currently "2".
        pub version: String,

        /// Global storage directory under which per-profile study
        /// folders are created. Each profile's data lives under
        /// `{storage_dir}/{profile_id}/`.
        pub storage_dir: PathBuf,

        /// Path to the SQLite database file. May be relative to
        /// `storage_dir` or absolute. Defaults to
        /// `{storage_dir}/bounce.db`.
        #[serde(default)]
        pub database_path: Option<PathBuf>,

        /// Optional Better Stack Logtail config. Disabled if absent.
        #[serde(default)]
        pub telemetry: Option<TelemetryConfig>,

        /// One or more profiles. Must be non-empty. Each profile's
        /// `id` must be unique. No two profiles may bind the same
        /// `(local.ip, local.port)` pair.
        pub profiles: Vec<ProfileConfig>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ProfileConfig {
        /// Stable, human-readable identifier. Used in logs, in the
        /// database `profile_id` column, and as the per-profile
        /// storage subdirectory name. Must match `[a-z0-9-]{1,32}`.
        pub id: String,

        /// Human-friendly display name, surfaced in the Tauri UI.
        #[serde(default)]
        pub name: Option<String>,

        pub local: LocalEndpoint,
        pub aurabox: AuraboxProfile,

        /// Number of upload worker tasks. Defaults to 2.
        #[serde(default = "default_workers")]
        pub workers: u32,

        /// Maximum concurrent C-STORE SCU sends for this profile's
        /// job poller. Defaults to 2 (matches current global cap).
        #[serde(default = "default_concurrent_sends")]
        pub max_concurrent_sends: u32,

        /// Study debounce window in seconds. Defaults to 10.
        #[serde(default = "default_debounce_secs")]
        pub debounce_secs: u64,

        /// Delete local study files after a successful upload.
        /// Defaults to false.
        #[serde(default)]
        pub delete_after_success: bool,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LocalEndpoint {
        /// IP to bind. `0.0.0.0` for all interfaces.
        pub ip: String,
        pub port: u16,
        pub ae_title: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AuraboxProfile {
        /// Aurabox API key. Carries region + tenancy + environment
        /// (e.g. aura_au_bounce_alice_TOKEN_production). Used as
        /// the Bearer credential for every Aurabox HTTP call and
        /// is the source of the base URL via the existing
        /// `region_from_api_key` / `mode_from_api_key` logic.
        pub api_key: String,

        /// TUS chunk size in megabytes. Defaults to 5 (matches
        /// today's hardcoded value).
        #[serde(default = "default_chunk_mb")]
        pub chunk_size_mb: u32,

        /// Aurabox poller interval in seconds. Defaults to 5.
        #[serde(default = "default_poll_secs")]
        pub poll_interval_secs: u64,
    }

A small `BounceConfig::validate()` method enforces: schema version
matches, at least one profile, unique profile IDs, unique
`(ip, port)` pairs across enabled profiles, AE titles non-empty and
within DICOM's 16-character limit, workers >= 1, and each
`api_key` parses into a region (via the existing
`region_from_api_key` rule).

A sample file lives at `crates/bounce-cli/examples/bounce.example.json`
and is referenced from the new `docs/CONFIGURATION.md` section.

### Phase 4: Per-profile receiver (M4)

`DICOMServer` is split. The generic accept-loop + PDU plumbing moves
into `dimse-gateway` as `Gateway`. The Aurabox-aware handler bodies
(C-STORE writes a file and queues an upload, C-FIND calls
`QueryApiClient::find_studies`, C-MOVE pulls bytes from Uhura via
WADO-RS and forwards via C-STORE SCU) move into
`bounce_aurabox::receiver` as concrete `CStoreHandler`,
`CFindHandler`, and `CMoveHandler` implementations. The constructor
takes a `ProfileRuntime` reference (see below) instead of a flat
`Config`. The bind address is `(profile.local.ip, profile.local.port)`;
the AE title is `profile.local.ae_title`. On `C-STORE` it writes files
under `{storage_dir}/{profile.id}/{study_uid}/{series_uid}/{sop_uid}.dcm`
and inserts/updates the `studies` row with `profile_id = profile.id`.

The shutdown channel becomes a `tokio::sync::broadcast::Sender<()>`
held by a new `ProfileRuntime` struct (see Phase 4) so that both the
receiver, the workers, and the poller can be cancelled in lockstep.

The database schema gains:

    ALTER TABLE studies ADD COLUMN profile_id TEXT NOT NULL DEFAULT '';
    ALTER TABLE uploads ADD COLUMN profile_id TEXT NOT NULL DEFAULT '';
    CREATE INDEX IF NOT EXISTS idx_studies_profile_id ON studies(profile_id);

This goes into a new migration file under
`crates/bounce-core/src/db/migrations/`. Existing rows get
`profile_id = 'default'` (set explicitly in the migration) so older
databases continue to round-trip; the upgraded binary writes one
"legacy" profile shim on first boot for any existing single-config
install (see Phase 6 / Tauri port).

### Phase 5: Per-profile worker pool (M5)

Replace `Transmission`'s `HashMap<study_uid, oneshot::Sender>`
debounce with the `study-pipeline` crate's bounded MPSC + N-worker
pool.

`study-pipeline` exposes (vendor-neutral):

    pub struct PipelineConfig {
        pub workers: u32,
        pub debounce: Duration,
        pub queue_capacity: usize,
    }

    #[async_trait]
    pub trait Shipper: Send + Sync + 'static {
        async fn ship(&self, study_uid: &str, zip_path: &Path)
            -> anyhow::Result<()>;
    }

    pub struct Pipeline { /* ... */ }

    impl Pipeline {
        pub fn start(
            cfg: PipelineConfig,
            shipper: Arc<dyn Shipper>,
            study_dir: Arc<dyn StudyDirectoryProvider>,
            shutdown: broadcast::Receiver<()>,
        ) -> Self;

        pub async fn enqueue(&self, study_uid: String);
    }

`bounce-aurabox` defines an `AuraboxShipper` (implements `Shipper`)
that captures the profile's `AuraboxClient`, runs `compress_study →
upload_init → tus_upload → upload_save → cleanup` for one study,
and updates the database. The receiver enqueues a study UID via
`pipeline.enqueue(study_uid)` after a successful C-STORE.

`ProfileRuntime` becomes:

    pub struct ProfileRuntime {
        pub profile: Arc<ProfileConfig>,
        pub database: Database,
        pub aurabox: Arc<AuraboxClient>,
        pub pipeline: Pipeline,
        pub event_sink: Arc<dyn EventSink>,
        pub shutdown_tx: broadcast::Sender<()>,
    }

`ProfileRuntime::start()` spawns: the `dimse-gateway` accept loop
(with the three Aurabox-aware handlers), the `study-pipeline` worker
pool, and the per-profile Aurabox poller (`aurabox-client` calling
`fetch_pending_jobs` / `fetch_pending_queries` on this profile's
API key every `aurabox.poll_interval_secs`). All spawned tasks
subscribe to the broadcast shutdown channel.

The Aurabox-specific `MAX_CONCURRENT_SENDS` constant in
`query/poller.rs` becomes the per-profile
`max_concurrent_sends` field (default 2) and is plumbed through.

### Phase 6: Extract the TUS primitive (M6)

The current `upload_via_tus` in `transmitter/transmission.rs` is
already substantially generic — it just happens to live inside
Aurabox code. M6 lifts it into the `tus-uploader` crate verbatim,
behind a small `TusClient` API:

    pub struct TusClient { /* reqwest::Client */ }

    pub struct TusUpload<'a> {
        pub endpoint: &'a str,
        pub bearer_token: Option<&'a str>,
        pub chunk_size: usize,
        pub metadata: &'a [(&'a str, &'a str)],
    }

    impl TusClient {
        pub fn new() -> Self;

        pub async fn upload(
            &self,
            file_path: &Path,
            opts: TusUpload<'_>,
            on_progress: impl Fn(u64, u64) + Send + Sync,
        ) -> anyhow::Result<()>;
    }

The Aurabox upload path inside `bounce-aurabox` is rewritten to:

1. Call `aurabox_client.upload_config()` to get the TUS endpoint,
   bearer token, bucket, and assembly_id.
2. Call `aurabox_client.upload_init(study_uid, signature, upload_id)`
   with the metadata Aurabox needs.
3. Construct a `TusUpload` describing the endpoint and the
   base64-shaped metadata the Aurabox TUS endpoint expects (`name`,
   `type`, `filename`, `fieldname`, `bucket`, `mode`, `upload_id`,
   `filetype`) and call `tus_client.upload(zip_path, opts, ...)`.
4. Call `aurabox_client.upload_save(upload_id, assembly_id, "complete")`.

The Aurabox handshake stays inside `bounce-aurabox`. The TUS chunked
HTTP cycle lives in `tus-uploader`. A future "Bounce for X" depends
on `tus-uploader` directly and does whatever pre/post handshake its
own product needs.

There is no `UploadTarget` trait, no factory, no second variant.
"Reusable" here means "extracted into a separate crate with a clean
API," not "polymorphic at runtime inside Bounce."

### Phase 7: Headless binary (M7)

`crates/bounce-cli/src/main.rs`:

    use clap::Parser;

    #[derive(Parser)]
    #[command(name = "bounced", version)]
    struct Cli {
        /// Path to the bounce.json config file.
        #[arg(short, long, value_name = "PATH")]
        config: std::path::PathBuf,

        /// Optional path to append structured logs to (in addition
        /// to stdout).
        #[arg(long, value_name = "PATH")]
        log_file: Option<std::path::PathBuf>,
    }

    #[tokio::main]
    async fn main() -> anyhow::Result<()> {
        let cli = Cli::parse();
        bounce_aurabox::telemetry::init_stdout_subscriber(cli.log_file.as_deref());
        let config = bounce_aurabox::BounceConfig::load_file(&cli.config)?;
        config.validate()?;
        let event_sink = Arc::new(bounce_aurabox::LoggingEventSink::default());
        let runtime = bounce_aurabox::Runtime::start(config, event_sink).await?;
        tokio::signal::ctrl_c().await?;
        runtime.shutdown().await?;
        Ok(())
    }

`bounce_aurabox::Runtime` is the top-level container: it owns one
`Database`, one `Arc<dyn EventSink>`, and one `ProfileRuntime` per
profile. `Runtime::start()` opens the database, runs migrations,
constructs each `ProfileRuntime`, and calls `start()` on each. It
returns a `Runtime` handle whose `shutdown()` sends the broadcast
shutdown signal to each profile and `.await`s their join handles.

The same `Runtime` is used from the Tauri entry point. The Tauri
binary supplies a `TauriEventSink` (which bridges to `app.emit`) and
otherwise drives the same code path. This means the Tauri UI keeps
working without duplicated logic.

### Phase 8: Vendor-neutrality probe (M9)

Add `crates/dimse-gateway/examples/echo_only.rs`. This is a small
program (target ≤ 50 lines) that:

1. Depends *only* on `dimse-gateway`, `tokio`, and `anyhow`.
2. Constructs a `Gateway` with a hardcoded bind address, AE title,
   and a single `CEchoHandler` implementation that logs each echo
   request.
3. Calls `gateway.run().await`.

The example is built as part of `cargo build --examples -p
dimse-gateway` and is exercised by `make check`. If a future change
accidentally pulls a `bounce-aurabox` or `aurabox-client` symbol
into `dimse-gateway`'s public API or default features, the example
fails to compile and CI catches it.

A parallel `crates/study-pipeline/examples/in_memory_shipper.rs`
asserts the same property for the pipeline crate: it implements a
trivial `Shipper` that just records study UIDs and asserts the
worker pool processes them in parallel.

Together these examples form a structural guarantee that the
primitive crates are decoupled from Aurabox.

### Phase 9: Tauri UI port (M8)

The Tauri front-end:

- Settings page is rewritten to render a list of profiles instead of a
  single set of fields. Each profile row exposes the local endpoint
  fields, the Aurabox API key, worker count, debounce window, TUS
  chunk size, poll interval, and delete-after-success toggle.
  Validation runs client-side and is re-checked by the backend
  before save.
- Save action calls a new IPC command, `save_config(BounceConfig)`,
  which writes `bounce.json` atomically (write-to-temp + rename) and
  then triggers `Runtime::reload()` on the backend. The runtime
  supports reloading by stopping any profiles whose binding has
  changed, starting any new ones, and leaving alone any profiles
  whose `local` and `aurabox` blocks are unchanged so the worker
  pool and any in-flight uploads survive.
- "Studies" and "Logs" pages get a profile filter dropdown driven by
  the `profile_id` column in the database.

This phase is the largest UI change, and the test plan splits it from
the headless work so each can be validated independently.

### Phase 10: Documentation and release (M10)

Update:

- `docs/ARCHITECTURE.md` — replace the "single AE, single port" prose
  in the "DICOM Endpoint Model" section with the per-profile model.
  Update the "Performance Characteristics" tables to note worker
  count and chunk size are now per-profile configurable.
- `docs/CONFIGURATION.md` — replace the plugin-store description with
  the `bounce.json` schema; include a worked example with two
  profiles.
- `docs/API.md` — add the new `save_config`, `reload_config`, and
  `list_profiles` Tauri commands; mark `receiver_start` /
  `receiver_stop` as legacy (they now start/stop *all* profiles).
- `README.md` — add a "Headless usage" section pointing operators at
  `bounced --config /path/to/bounce.json`.
- `CHANGELOG.md` — add a `## [2.0.0]` entry describing the headless
  binary, profiles, worker pools, and the breaking config change.
- Run `make version V=2.0.0`.

## Concrete Steps

These are the exact commands to run, in order. They assume the
working directory is the repository root unless otherwise stated.

### Step 1: Snapshot the current state

    git status
    git log --oneline -10
    cargo --version
    rustc --version

Expected: clean working tree (or only the documentation changes in this
plan), recent commits including the 1.5.0 release tag, Cargo
≥ 1.80 (matching `rust-version = "1.80"` in `src-tauri/Cargo.toml`).

### Step 2: Build & test the existing code as a regression baseline

    make test
    npm install
    npm run build

Record the test counts that pass today. Any post-refactor regression
will show up as a delta against this baseline.

### Step 3: Convert `src-tauri/` to a Cargo workspace (M1)

Inside `src-tauri/`, replace the existing `Cargo.toml` with a virtual
workspace manifest that lists the seven member crates:

    [workspace]
    members = [
        "crates/dimse-gateway",
        "crates/study-pipeline",
        "crates/tus-uploader",
        "crates/aurabox-client",
        "crates/bounce-aurabox",
        "crates/bounce-cli",
        "crates/bounce-tauri",
    ]
    resolver = "2"

For M1, move the existing `src-tauri/src/` tree wholesale to
`src-tauri/crates/bounce-aurabox/src/`. The four primitive crates
start as empty `lib.rs` files with `Cargo.toml` declarations; their
contents are populated in M2 (Tauri-decoupling), M4 (DIMSE handlers
move out), M5 (pipeline moves out), and M6 (TUS moves out)
respectively. The `aurabox-client` crate likewise starts empty and
absorbs the contents of `src-tauri/src/aura/` in M2. Create
`crates/bounce-tauri/src/main.rs` containing the existing
`fn main()` (IPC commands and Tauri builder) with `use
bounce_aurabox::...` paths. Create `crates/bounce-cli/src/main.rs`
as a stub that prints `"bounced placeholder"` — fleshed out in M7.
Update `tauri.conf.json` so `mainBinaryName` points at the
`bounce-tauri` binary target. Re-run `make build` and verify the
desktop bundle still builds.

### Step 4: Break the Tauri dependency from `bounce-aurabox` (M2)

Replace every `AppHandle` usage in `bounce-aurabox` with explicit
constructor parameters or the `EventSink` callback trait introduced
in Phase 2. Move `src-tauri/src/aura/` into `crates/aurabox-client`.
Move the Logtail integration into `bounce-aurabox::telemetry`.
Confirm:

    cd src-tauri && cargo build -p bounce-aurabox
    cd src-tauri && cargo test -p bounce-aurabox
    cd src-tauri && cargo tree -p bounce-aurabox | grep -E '^tauri'

The last command must print nothing. The same `grep` against each
primitive crate (`-p dimse-gateway`, `-p study-pipeline`,
`-p tus-uploader`) must also be empty, and `cargo tree -p
dimse-gateway | grep -E 'aurabox|bounce-'` must be empty (no
back-reference from a primitive into a product crate).

### Step 5: Implement the new config schema (M3)

Add `bounce-core::config::{BounceConfig, ProfileConfig, ...}` with
`serde` derive and a `validate()` method. Add round-trip tests that
load `crates/bounce-cli/examples/bounce.example.json` and assert the
expected profile count, IDs, and validation results. Sample contents:

    {
      "version": "2",
      "storage_dir": "/var/lib/bounce",
      "telemetry": null,
      "profiles": [
        {
          "id": "clinic-a",
          "name": "Clinic A (Aurabox primary)",
          "local": { "ip": "0.0.0.0", "port": 11112, "ae_title": "BOUNCE" },
          "aurabox": {
            "api_key": "aura_au_bounce_alice_PLACEHOLDER_production",
            "chunk_size_mb": 5,
            "poll_interval_secs": 5
          },
          "workers": 4,
          "max_concurrent_sends": 2,
          "debounce_secs": 10,
          "delete_after_success": true
        },
        {
          "id": "clinic-b",
          "name": "Clinic B (separate Aurabox tenancy)",
          "local": { "ip": "0.0.0.0", "port": 11113, "ae_title": "BOUNCE-B" },
          "aurabox": {
            "api_key": "aura_au_bounce_bob_PLACEHOLDER_production",
            "chunk_size_mb": 10,
            "poll_interval_secs": 5
          },
          "workers": 2,
          "max_concurrent_sends": 2,
          "debounce_secs": 15,
          "delete_after_success": false
        }
      ]
    }

### Step 6: Move DIMSE wire code into `dimse-gateway`; add `profile_id` columns (M4)

Lift the accept loop, PDU parsing, and ServerAssociationOptions
construction from `bounce-aurabox/src/receiver/dicom_server.rs`
into `crates/dimse-gateway/src/`. Define the `CStoreHandler`,
`CFindHandler`, `CMoveHandler`, `CEchoHandler` traits. Implement
them concretely in `bounce-aurabox::receiver::handlers` against the
existing Aurabox-aware behaviour. Write the SQLite migration
described in Phase 4 (`ALTER TABLE studies ADD COLUMN profile_id …`).
Confirm:

    cd src-tauri && cargo test -p dimse-gateway
    cd src-tauri && cargo test -p bounce-aurabox receiver

Receiver tests pass with the new shape. Legacy single-profile
behaviour is reproduced exactly when `profiles` has length 1.

### Step 7: Move the worker pool into `study-pipeline` (M5)

Lift the debounce + worker shape from
`bounce-aurabox/src/transmitter/transmission.rs` into
`crates/study-pipeline/src/`. Define the `Shipper` trait. Write
`AuraboxShipper` in `bounce-aurabox/src/transmitter/`. Verify with a
new integration test in `crates/study-pipeline/tests/worker_pool.rs`
that two studies pushed back-to-back into a 2-worker pool are
processed concurrently (measured by start/finish timestamps in a stub
`Shipper` that sleeps for 500 ms). The test must fail if work is
serialised. The same crate's `examples/in_memory_shipper.rs` is the
M9 vendor-neutrality probe.

### Step 8: Extract the TUS uploader (M6)

Move the body of `upload_via_tus` into
`crates/tus-uploader/src/lib.rs` behind the `TusClient` API. Update
`bounce-aurabox`'s upload path to call `tus_client.upload(...)`
after the `upload_init` handshake and before the `upload_save`
finalisation. Add unit tests against a `mockito` HTTP server in
`crates/tus-uploader/tests/upload.rs`:

- POST creates the resource with the right `Upload-Length`,
  `Upload-Metadata`, and `Authorization` headers.
- PATCH chunks carry the right `Upload-Offset` and chunk size.
- The progress callback fires at every chunk boundary.

Also re-run the existing Aurabox upload integration tests inside
`bounce-aurabox` against `mockito` to confirm the end-to-end Aurabox
flow still works after the extraction.

### Step 9: Wire up `bounced` (M7)

Flesh out `crates/bounce-cli/src/main.rs` per Phase 6. Build:

    cd src-tauri && cargo build -p bounce-cli --release
    ls -lh target/release/bounced

Run an end-to-end test using the existing Orthanc PACS Docker config in
`docker/`:

    docker-compose -f docker/docker-compose.yml up -d
    target/release/bounced --config docs/EXECPLAN-2.0-sample.json &
    storescu -aec BOUNCE 127.0.0.1 11112 path/to/sample.dcm
    storescu -aec BOUNCE-R 127.0.0.1 11113 path/to/sample.dcm

Expect `bounced` to log a "Stored …" line for each profile, a
"Scheduling study push" line, and finally "Upload completed
successfully" for each profile's upload target.

### Step 10: Port the Tauri UI (M8)

Implement the UI changes described in Phase 7. Run:

    make dev

and exercise the Settings page: add a profile, save, see the new
profile come up in the runtime status panel without restarting the
app. Repeat the dual-profile DICOM test from Step 9 against the
desktop app.

### Step 11: Update documentation and version (M10)

Edit the documentation files listed in Phase 8. Run:

    make version V=2.0.0
    git diff package.json src-tauri/crates/*/Cargo.toml src-tauri/tauri.conf.json

Expect every workspace member, `package.json`, and `tauri.conf.json`
to show `2.0.0`. Add the CHANGELOG entry and commit.

## Validation and Acceptance

Each of the following must be observably true at the end of the
indicated milestone. "Observably" means a human can run a command and
see a transcript that matches.

### M1 acceptance

- `cd src-tauri && cargo build` builds all three workspace members.
- `cd src-tauri && cargo test` reports the same number of tests
  passing as before the workspace split.

### M2 acceptance

- `grep -RE "^tauri[[:space:]]*=" src-tauri/crates/bounce-aurabox/Cargo.toml
  src-tauri/crates/dimse-gateway/Cargo.toml
  src-tauri/crates/study-pipeline/Cargo.toml
  src-tauri/crates/tus-uploader/Cargo.toml
  src-tauri/crates/aurabox-client/Cargo.toml` returns no matches.
- `cd src-tauri && cargo build -p bounce-aurabox` succeeds without
  Tauri in the dependency tree (verified with `cargo tree -p
  bounce-aurabox | grep -E '^tauri'` returning nothing).
- For each primitive crate, `cargo tree -p <crate> | grep -E
  'aurabox|bounce-'` returns nothing (no back-reference into product
  code).

### M3 acceptance

- `cd src-tauri && cargo test -p bounce-aurabox config::tests::`
  passes and exercises:
  - Loading the bundled `bounce.example.json` succeeds.
  - Duplicate profile IDs fail validation.
  - Two profiles binding the same `(ip, port)` fail validation.
  - `workers = 0` fails validation.
  - A profile whose `api_key` does not parse a region fails
    validation (re-uses the existing `region_from_api_key` rule).

### M4 acceptance

- Running the existing Rust receiver tests under the new
  `ReceiverServer` path passes with no behavioural regression.
- A fresh `bounce.db` written by the new code shows a `profile_id`
  column on `studies` and `uploads` (`sqlite3 bounce.db ".schema
  studies"`).

### M5 acceptance

- The integration test in
  `crates/study-pipeline/tests/worker_pool.rs` passes. The test
  asserts that with `workers = 2`, two 500 ms jobs finish in under
  800 ms total (proving concurrency); and that with `workers = 1`,
  the same two jobs take at least 1 s total.

### M6 acceptance

- `crates/tus-uploader/tests/upload.rs` passes against a mockito
  TUS server (POST creates with the right `Upload-Length` and
  `Upload-Metadata`, PATCH chunks with the right offsets).
- The existing Aurabox-flow integration tests inside
  `bounce-aurabox` still pass after the extraction.
- `cargo tree -p tus-uploader | grep -E 'aurabox|bounce-'` returns
  nothing, confirming the TUS crate is vendor-neutral.

### M7 acceptance

Run the end-to-end transcript:

    target/release/bounced --config ./docs/EXECPLAN-2.0-sample.json

Expected log lines (timestamps elided):

    INFO bounce_aurabox: opened database at /var/lib/bounce/bounce.db
    INFO bounce_aurabox: starting profile clinic-a
    INFO dimse_gateway: listening on 0.0.0.0:11112 (ae=BOUNCE)
    INFO bounce_aurabox: starting profile clinic-b
    INFO dimse_gateway: listening on 0.0.0.0:11113 (ae=BOUNCE-B)
    INFO bounce_aurabox: 2 profiles ready

Then, after `storescu -aec BOUNCE 127.0.0.1 11112 sample.dcm`:

    INFO dimse_gateway: new association from STORESCU (profile=clinic-a)
    INFO bounce_aurabox::receiver: stored .../sample.dcm (profile=clinic-a)
    INFO study_pipeline: scheduling study push 1.2.3.4 (profile=clinic-a)
    INFO bounce_aurabox::transmitter: upload completed (profile=clinic-a)

### M8 acceptance

- The desktop app launches via `make dev`.
- The Settings page renders the list of profiles from `bounce.json`.
- Editing and saving a profile updates the JSON on disk; the runtime
  picks up the change without a process restart.
- The Studies page filter dropdown lists every distinct `profile_id`
  in the database.

### M9 acceptance

- `cargo build --examples -p dimse-gateway` succeeds.
- `cargo build --examples -p study-pipeline` succeeds.
- `target/debug/examples/echo_only` runs and accepts a C-ECHO from
  `echoscu -aec ANY 127.0.0.1 11112` with status `0x0000`.
- Repeated for the in-memory shipper example for `study-pipeline`.
- `make check` includes these example builds so any future
  regression is caught immediately.

### M10 acceptance

- For every workspace member `M`,
  `grep '^version' src-tauri/crates/M/Cargo.toml` shows `2.0.0`.
- `grep '^version' package.json` and `grep '"version"'
  src-tauri/tauri.conf.json` show `2.0.0`.
- The CHANGELOG `[2.0.0]` entry calls out the headless binary,
  per-tenancy profiles, worker pools, the new schema, the new crate
  layout, and that the v1 `store.json` format is no longer read.

## Idempotence and Recovery

- Every JSON config write performed by the Tauri UI uses
  "write to `bounce.json.tmp` → fsync → rename" semantics. A crash
  mid-write therefore leaves either the old or the new file intact,
  never a half-written one.
- Database migrations are additive (`ALTER TABLE ... ADD COLUMN`) and
  bracketed by SQLx's migration tracker so re-running them is a no-op.
- `bounced` accepts `SIGTERM` and `SIGINT` and exits cleanly after
  all in-flight uploads finish or time out (configurable via
  `BOUNCE_SHUTDOWN_TIMEOUT_SECS`, default 30).
- The first-boot config migration described in M4 is gated on the
  absence of `bounce.json`: if a `bounce.json` already exists, the
  old Tauri plugin-store contents are ignored. This makes upgrades
  re-runnable without risk of clobbering an intentionally-edited
  `bounce.json`.

## Artifacts and Notes

Sample log output from a planned worker pool test (Phase 4) — kept here
for reference when the test is implemented:

    test worker_pool::two_jobs_run_concurrently_with_two_workers ...
        spawned profile worker-pool-test with workers=2
        pushed StudyJob(study_uid="1.2.3.4.A") at t=0ms
        pushed StudyJob(study_uid="1.2.3.4.B") at t=1ms
        finished StudyJob(study_uid="1.2.3.4.A") at t=512ms
        finished StudyJob(study_uid="1.2.3.4.B") at t=514ms
        elapsed: 515ms (must be < 800ms)
    test result: ok. 1 passed; 0 failed

Sample failure transcript for the validation `profiles must have
unique (ip,port)` rule:

    error: bounce.json failed validation: profiles "clinic-a" and
    "clinic-b" both bind 0.0.0.0:11112

## Interfaces and Dependencies

This section is prescriptive. At the end of each milestone the
listed names and signatures must exist in the listed locations.

In `crates/dimse-gateway/src/lib.rs`:

    pub struct Gateway { /* ... */ }

    pub struct GatewayConfig {
        pub bind: std::net::SocketAddr,
        pub ae_title: String,
        pub abstract_syntaxes: Vec<&'static str>,
        pub strict: bool,
        pub promiscuous: bool,
        pub max_pdu_length: u32,
    }

    #[async_trait::async_trait]
    pub trait CStoreHandler: Send + Sync + 'static {
        async fn handle(&self, ctx: CStoreContext<'_>)
            -> anyhow::Result<CStoreResponse>;
    }

    #[async_trait::async_trait]
    pub trait CFindHandler: Send + Sync + 'static {
        async fn handle(&self, ctx: CFindContext<'_>) -> anyhow::Result<()>;
    }

    #[async_trait::async_trait]
    pub trait CMoveHandler: Send + Sync + 'static {
        async fn handle(&self, ctx: CMoveContext<'_>) -> anyhow::Result<()>;
    }

    #[async_trait::async_trait]
    pub trait CEchoHandler: Send + Sync + 'static {
        async fn handle(&self, ctx: CEchoContext<'_>) -> anyhow::Result<()>;
    }

    impl Gateway {
        pub fn builder(cfg: GatewayConfig) -> GatewayBuilder;
        pub async fn run(self, shutdown: broadcast::Receiver<()>)
            -> anyhow::Result<()>;
    }

In `crates/study-pipeline/src/lib.rs`:

    pub struct PipelineConfig {
        pub workers: u32,
        pub debounce: Duration,
        pub queue_capacity: usize,
    }

    #[async_trait::async_trait]
    pub trait Shipper: Send + Sync + 'static {
        async fn ship(&self, study_uid: &str, zip_path: &Path)
            -> anyhow::Result<()>;
    }

    pub trait StudyDirectoryProvider: Send + Sync + 'static {
        fn study_dir(&self, study_uid: &str) -> PathBuf;
        fn study_archive(&self, study_uid: &str) -> PathBuf;
    }

    pub struct Pipeline { /* ... */ }

    impl Pipeline {
        pub fn start(
            cfg: PipelineConfig,
            shipper: Arc<dyn Shipper>,
            dirs: Arc<dyn StudyDirectoryProvider>,
            shutdown: broadcast::Receiver<()>,
        ) -> Self;
        pub async fn enqueue(&self, study_uid: String);
    }

In `crates/tus-uploader/src/lib.rs`:

    pub struct TusClient { /* reqwest::Client wrapper */ }

    pub struct TusUpload<'a> {
        pub endpoint: &'a str,
        pub bearer_token: Option<&'a str>,
        pub chunk_size_bytes: usize,
        pub metadata: &'a [(&'a str, &'a str)],
    }

    impl TusClient {
        pub fn new() -> Self;
        pub async fn upload(
            &self,
            file_path: &Path,
            opts: TusUpload<'_>,
            on_progress: impl Fn(u64, u64) + Send + Sync,
        ) -> anyhow::Result<()>;
    }

In `crates/aurabox-client/src/lib.rs`:

    pub struct AuraboxClient { /* reqwest::Client + api_key + endpoint */ }

    impl AuraboxClient {
        pub fn new(api_key: String) -> Self;
        pub async fn upload_config(&self) -> anyhow::Result<UploaderConfig>;
        pub async fn upload_init(&self, ...) -> anyhow::Result<Value>;
        pub async fn upload_save(&self, ...) -> anyhow::Result<Value>;
        pub async fn fetch_services(&self) -> anyhow::Result<ServicesResponse>;
        pub async fn fetch_pending_jobs(&self) -> anyhow::Result<PendingJobsResponse>;
        pub async fn fetch_pending_queries(&self) -> anyhow::Result<PendingQueriesResponse>;
        // …one method per existing Aurabox endpoint, all moved verbatim
        // from src-tauri/src/aura/.
    }

In `crates/bounce-aurabox/src/config.rs`: the `BounceConfig`,
`ProfileConfig`, `LocalEndpoint`, and `AuraboxProfile` types as
defined in Phase 3, plus:

    impl BounceConfig {
        pub fn load_file(path: &std::path::Path) -> anyhow::Result<Self>;
        pub fn save_file(&self, path: &std::path::Path) -> anyhow::Result<()>;
        pub fn validate(&self) -> anyhow::Result<()>;
    }

In `crates/bounce-aurabox/src/runtime.rs`:

    pub trait EventSink: Send + Sync + 'static {
        fn emit(&self, topic: &str, payload: serde_json::Value);
    }

    pub struct Runtime { /* ... */ }
    pub struct ProfileRuntime { /* ... */ }

    impl Runtime {
        pub async fn start(
            cfg: BounceConfig,
            event_sink: Arc<dyn EventSink>,
        ) -> anyhow::Result<Self>;
        pub async fn reload(&self, cfg: BounceConfig) -> anyhow::Result<()>;
        pub async fn shutdown(self) -> anyhow::Result<()>;
        pub fn profiles(&self) -> &[Arc<ProfileRuntime>];
    }

In `crates/bounce-cli/src/main.rs`: the `Cli` struct and `main`
function shown in Phase 7.

In `crates/bounce-tauri/src/main.rs`: the existing IPC commands,
rewritten to take a `tauri::State<Arc<Runtime>>` parameter instead of
constructing `Transmission` / `Database` ad-hoc.

External crates added:

- `clap` (CLI parsing) in `bounce-cli`.
- `tracing` and `tracing-subscriber` in `bounce-aurabox`.
- `async-trait` in `dimse-gateway`, `study-pipeline`, and
  `bounce-aurabox`.
- `tokio` continues to be a `full` features dependency in
  `bounce-aurabox`; the primitive crates pull only the subset of
  Tokio features they actually use (`net`, `sync`, `time`, `fs`).

Crates removed from `bounce-aurabox` (still used by `bounce-tauri`):

- `tauri`, `tauri-plugin-store`, `tauri-plugin-log`,
  `tauri-plugin-process`, `tauri-plugin-shell`, `tauri-plugin-dialog`,
  `tauri-plugin-opener`, `tauri-plugin-os`, `tauri-plugin-updater`.

What is deliberately **not** present:

- No `Backend` trait, no `UploadTarget` trait, no
  `dyn` upload-target abstraction inside Bounce. The TUS uploader is
  a concrete `TusClient`. The Aurabox HTTP client is a concrete
  `AuraboxClient`. They are reusable because they live in their own
  crates, not because a runtime selects between two implementations.

## Revisions to this plan

Every material change to this plan must be appended below with a date,
a short description of what changed, and why. The plan must always
remain re-readable top-to-bottom as a single self-contained guide.

- 2026-05-26 — Initial draft authored by planner. Captures M0–M9 and
  every decision made while reading the existing source tree.

- 2026-05-26 — Reframed reusability strategy. The first draft proposed
  an `UploadTarget` trait inside Bounce with `Aurabox` and
  `CustomTus` variants and a `type:` tag in the JSON schema. The user
  clarified that Bounce stays Aurabox-only and that reusability
  should be delivered through crate boundaries so a *separate* future
  product can be built on the same primitives. The plan now splits
  the workspace into three layers (primitives → product → app) with
  seven crates: `dimse-gateway`, `study-pipeline`, `tus-uploader`
  (primitives); `aurabox-client`, `bounce-aurabox` (product);
  `bounce-cli`, `bounce-tauri` (apps). The `UploadTarget` trait, the
  `CustomTus` variant, and the `type:` JSON tag are removed. A new
  M9 milestone validates the primitive crates' vendor-neutrality via
  small example binaries that depend only on the primitive crates,
  `tokio`, and `anyhow`. The schema is reshaped: the `upload` block
  with two variants becomes a flat `aurabox` block with `api_key`,
  `chunk_size_mb`, and `poll_interval_secs`. Sample JSON, Decision
  Log, Plan of Work, Concrete Steps, Validation, and Interfaces are
  all updated.
