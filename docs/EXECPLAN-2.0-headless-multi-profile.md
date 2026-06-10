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
runs an uploader capped at two concurrent uploads by a process-global
semaphore, and reads all of its operational configuration from a
Tauri-managed JSON store edited through the Next.js UI. There is no way
to run it without a graphical session, and there is no way to have one
machine accept DICOM traffic on behalf of more than one logical
"tenant" or upload destination.

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
  callbacks and a pluggable association `AccessPolicy`),
  `study-pipeline` (per-profile debounce + bounded MPSC + N-worker
  pool, plus the pure capped-exponential-backoff arithmetic), and
  `tus-uploader` (chunked TUS POST/PATCH client taking endpoint,
  token, chunk size, and metadata as inputs). They are not used
  polymorphically inside Bounce; they are the layered building
  blocks Bounce composes against.
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
  `study-pipeline` replace the debounce-plus-global-semaphore
  uploader; the retry scheduler and claim/attempt/record lifecycle
  are preserved per profile; the `UPLOAD_SEMAPHORE` static is
  deleted; load test with two concurrent uploads shows both
  progressing.
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
  one of two front-ends to the same `bounce-aurabox` runtime.
  Rationale: The user statement says 2.0 "needs to" run headless and
  describes profile semantics; it does not say the desktop UI is
  going away. Keeping the UI working preserves the existing install
  base and avoids a coupled UI rewrite landing inside an already large
  backend refactor. If the user later confirms the UI should be
  dropped, removing `bounce-tauri` is a single-crate deletion.
  Date/Author: 2026-05-26 / planner.

- Decision: One shared SQLite database file (`bounce.db`) for all
  profiles. The `studies` and `upload_attempts` tables gain a
  `profile_id` text column, and the uniqueness constraint on
  `studies` changes from `study_uid` alone to the composite
  `(profile_id, study_uid)`. Queries that previously had no scope are
  now scoped by `profile_id`.
  Rationale: Per-profile databases would multiply file-handle usage
  and complicate the UI's "list everything I've ever received" view.
  A single database with a `profile_id` discriminator is the smallest
  change. The composite key is forced by a real scenario: the same
  modality can be configured to send the same study to two profiles
  on one host (two tenancies receiving the same patient), and study
  instance UIDs are globally stable, so `study_uid` alone would
  collide. SQLite cannot alter a UNIQUE constraint in place, so this
  migration is a guarded table rebuild (create `studies_new` with the
  composite key, copy rows with `profile_id = 'default'`, drop, and
  rename) rather than a simple `ADD COLUMN`.
  Date/Author: 2026-06-10 / planner (supersedes the 2026-05-26 entry
  which predated the `upload_attempts` table and assumed an
  additive-only migration).

- Decision: The upload retry/recovery machinery added in 1.6
  (status machine on `studies`, per-attempt `upload_attempts` audit
  rows, exponential backoff, the 30-second retry scheduler, and the
  disk-space guard) is preserved in 2.0 and becomes profile-aware
  rather than being replaced by the worker pool. Specifically:
  the pure backoff math (`RetryPolicy`, `next_retry_delay`) moves
  into `study-pipeline` as a vendor-neutral module; the DB-driven
  retry scheduler stays in `bounce-aurabox` (it is coupled to the
  Bounce schema) but scans per profile and feeds due studies into
  that profile's pipeline queue; the atomic claim
  (`claim_study_for_upload`) and attempt recording stay inside the
  `AuraboxShipper`, so every upload — debounced, retried, or manual —
  flows through one code path. The process-global static
  `UPLOAD_SEMAPHORE` (`MAX_CONCURRENT_UPLOADS = 2`) is deleted; the
  per-profile worker count *is* the concurrency limit.
  Rationale: The retry subsystem is recent, tested, operator-visible
  (the Transactions view is built on it), and orthogonal to
  profiles — discarding it to "simplify" the worker-pool refactor
  would regress 1.6–1.9 behaviour. The global semaphore, by
  contrast, is exactly the kind of cross-profile shared state
  profiles exist to remove, and it is a static `OnceLock` global
  that could never be per-profile anyway.
  Date/Author: 2026-06-10 / planner.

- Decision: AE-title allow-listing (AURA-2296) becomes per-profile.
  Each profile's SCP accepts only calling AE titles present in *that
  profile's* Aurabox services catalog; `permissive_mode` becomes a
  per-profile boolean that switches the policy to accept-any and
  stores received instances without queueing them for upload. The
  services catalog cache moves out of the Tauri plugin-store into a
  `pacs_services` table in `bounce.db` keyed by `profile_id`
  (refreshed by each profile's poller on the existing cadence).
  `dimse-gateway` exposes the acceptance decision as a generic
  `AccessPolicy` hook on `GatewayConfig` (the same seam as
  `dicom_ul`'s `AccessControl` trait); the Aurabox-aware policy that
  reads the per-profile catalog lives in `bounce-aurabox`.
  Rationale: The allow-list is derived from the services catalog,
  which is tenancy-scoped — two tenancies on one host have different
  registered PACS, so a global list would let clinic B's modality
  store into clinic A's profile. The cache must leave the
  plugin-store regardless (the plugin-store is being removed); the
  database is the natural home because the cache must be shared
  between the headless and Tauri front-ends and is already
  per-profile keyed data.
  Date/Author: 2026-06-10 / planner.

- Decision: Retry policy (`max_attempts`, `base_seconds`,
  `cap_seconds`) is per-profile with defaults matching today's
  values (10 / 30 / 3600). Disk thresholds (`disk_warn_mb`,
  `disk_critical_mb`) stay **global** in the config root.
  Rationale: Retry aggressiveness is a per-tenancy service decision
  (a research tenancy may tolerate week-long retry tails; a clinical
  one may want fast terminal failure and human follow-up). Disk
  space is a property of the shared storage volume — per-profile
  thresholds against one volume would just mean the lowest value
  wins, which is the same behaviour with extra configuration
  surface.
  Date/Author: 2026-06-10 / planner.

- Decision: Single-instance enforcement stays a Tauri-app concern.
  `bounce-tauri` keeps `tauri-plugin-single-instance` (AURA-2291).
  The headless `bounced` does not get an equivalent lock file;
  instead, `Runtime::start` treats a bind failure on any profile's
  port as fatal, reports which profile and port collided, and exits
  non-zero before any other profile is left half-started (bind all
  listeners first, then start workers/pollers).
  Rationale: On a server, running two `bounced` processes with
  *different* configs is legitimate (e.g. blue/green on different
  ports), so a process-level mutex would be wrong. The port bind is
  the actual contended resource and already serialises access; what
  matters is failing loudly and atomically rather than partially
  starting.
  Date/Author: 2026-06-10 / planner.

- Decision: The JSON config is the single source of truth at runtime;
  the Tauri plugin-store is removed from the product crate. The UI
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

The current backend (version 1.9.1 at the time this section was last
verified) is a single Cargo crate, `app`, declared in
`src-tauri/Cargo.toml`. Its `main.rs` builds a `tauri::Builder`,
registers IPC commands, and wires up background tasks. The relevant
modules are:

- `src-tauri/src/store/config.rs` — `Config` struct loaded synchronously
  from the Tauri plugin-store. Holds `api_key`, `port`, `ip_address`,
  `ae_title`, `base_dir`, `delete_after_success`, `send_logs`,
  `permissive_mode`, and five retry/disk knobs:
  `max_upload_attempts` (default 10), `retry_base_seconds` (30),
  `retry_cap_seconds` (3600), `disk_warn_mb` (2048),
  `disk_critical_mb` (512). Singular fields — there is no concept of
  more than one of any of them. The frontend additionally persists a
  `start_receiver_on_start` preference and a transient
  `_resume_running` flag directly in the plugin-store.
- `src-tauri/src/store/pacs_cache.rs` — caches the Aurabox services
  catalog (the list of remote PACS the tenancy knows about) in the
  plugin-store so the UI renders offline. Since AURA-2296 this cache
  is also the **SCP allow-list**: the receiver only accepts
  associations whose calling AE title appears in the cached services
  list.
- `src-tauri/src/store/disk.rs` — disk-space guard. `check_disk()`
  reads free space on the storage volume via the `fs2` crate;
  `warn_if_low()` emits a `disk-warning` event when below
  `disk_warn_mb` and returns `true` (pause new uploads) when below
  `disk_critical_mb`.
- `src-tauri/src/receiver/dicom_server.rs` — `DICOMServer::start()`
  binds a single `tokio::net::TcpListener` to `(config.ip_address,
  config.port)`, then loops on `listener.accept()` and processes each
  association serially inside `run_store_sync()`. Holds an
  `Option<AppHandle>` that it uses to emit `queue-study` and `log`
  events to the UI. Association acceptance is governed by a
  `CallerPolicy` enum implementing `dicom_ul`'s `AccessControl`
  trait: `AcceptRegistered(list)` admits only calling AE titles in
  the cached PACS services list (an empty list rejects everyone);
  `AcceptAny` is used when `permissive_mode` is "yes". In permissive
  mode received instances are stored to disk but **not** queued for
  upload.
- `src-tauri/src/receiver/server.rs` — `start()` and `stop()` manage
  the receiver's lifecycle. `ServerState` (stashed in Tauri-managed
  `Arc<Mutex<…>>`) now holds three shutdown senders: the receiver's,
  the query poller's, and the retry scheduler's. Starting the
  receiver also starts the poller and the retry scheduler; stopping
  it shuts down all three.
- `src-tauri/src/transmitter/transmission.rs` — `Transmission` owns a
  `reqwest::Client`, a `HashMap` of debounce timers
  (`scheduled_studies`), and an `AppHandle`. `schedule_study_push()`
  runs the 10-second debounce, then routes through
  `attempt_upload()`, which: acquires a process-global static
  `UPLOAD_SEMAPHORE` (`MAX_CONCURRENT_UPLOADS = 2`); refuses to
  start when disk space is critical; atomically claims the study via
  `Database::claim_study_for_upload` (eligible states: IN_PROGRESS,
  QUEUED, RETRYING); runs `send_study()` (compress → Aurabox
  `upload_init` → `upload_via_tus` → `upload_save`); and records the
  outcome — `mark_attempt_success` or `mark_upload_failed` with an
  exponential-backoff `next_retry_at` computed by
  `transmitter/backoff.rs`. A study whose attempts exceed
  `max_upload_attempts` is marked terminally FAILED.
- `src-tauri/src/transmitter/backoff.rs` — pure functions:
  `RetryPolicy` (max attempts, base delay, cap) and
  `next_retry_delay(attempt_no, policy)` implementing capped
  exponential backoff. No I/O, no Tauri.
- `src-tauri/src/transmitter/retry_scheduler.rs` — background loop
  (30-second cadence) that scans the database for studies due for an
  attempt (`find_due_for_retry`, batch limit 100) and dispatches each
  through `Transmission::attempt_upload`, tracking in-flight UIDs to
  avoid redundant dispatch. Mirrors the query poller's
  shutdown-channel shape.
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
  `AppHandle`. The `studies` table carries upload-lifecycle columns
  (`attempts`, `last_attempt_at`, `next_retry_at`, `last_error`) and
  a per-attempt audit table `upload_attempts` (one row per attempt:
  `attempt_no`, `upload_id`, `status`, `error`, timings) feeds the
  UI's Transactions view.
- `src-tauri/src/logger.rs` — Custom `log::Log` implementation that
  batches log lines and POSTs them to Better Stack Logtail. Wired up
  in `main.rs::setup_logger_with_config()`.

The frontend (`app/`) reads and writes the plugin-store via Tauri IPC
and listens for events (`log`, `running`, `running-details`,
`current-studies`, `upload-progress`, `queue-study`,
`transactions-updated`, `disk-warning`). It has a Transactions view
(per-attempt history with Retry and Stop buttons), a dashboard status
summary, and a Studies table with search and bulk actions. It does
not currently know anything about profiles.

The Tauri commands registered today (all of which must keep working
after the port): `receiver_start`, `receiver_stop`, `reset_app`,
`send_log`, `send_study`, `retry_study`, `cancel_retry`,
`study_upload_attempts`, `delete_study`, `bulk_send_studies`,
`bulk_delete_studies`, `delete_all_studies`, `api_start_upload`,
`current_studies`, `current_upload_attempts`, `dashboard_stats`,
`cfind_query`, `list_pacs_services`, `refresh_pacs_services`,
`echo_pacs_service`, `show_window`, `update_send_logs`,
`verify_connectivity`, `request_exit`.

The Tauri builder also registers `tauri-plugin-single-instance`
(prevents a second app instance fighting over the DICOM port,
AURA-2291), `tauri-plugin-autostart` (start-on-login), and
`tauri-plugin-updater` (automatic updates with a restart window that
preserves receiver state via the `_resume_running` flag). On launch,
the receiver auto-starts when `start_receiver_on_start` is "yes" or
`_resume_running` was set by a prior in-app relaunch (AURA-2290).

Key coupling points between "Tauri" and "business logic" today:

1. **Config loading.** `Config::load(AppHandle)` reads from
   `app.store("store.json")`. Everywhere a piece of code needs config,
   it calls `crate::load_config(app)` which forwards to that.
2. **Event emission.** Server status, log messages, study progress,
   transaction updates, and disk warnings are all delivered via
   `app_handle.emit(...)`. Without an `AppHandle`, there is no place
   for the events to go.
3. **Database location.** `Database::new(AppHandle)` resolves the path
   via `app_handle.path().app_data_dir()`.
4. **Logger backend.** `tauri-plugin-log` is registered on the Tauri
   builder; in a headless world it would not exist.
5. **PACS cache / SCP allow-list.** `store/pacs_cache.rs` persists the
   services catalog in the plugin-store, and the receiver reads it at
   association time to build its `CallerPolicy`. Removing the
   plugin-store therefore requires a new home for this cache.

The refactor's job is to make those five coupling points pluggable and
to introduce profile-aware container types around the existing logic.
The existing DICOM, TUS, SQLite, retry, and disk-guard code is largely
correct and can be re-used in place once it stops reaching for
`AppHandle`.

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
  abstract-syntax list, an association `AccessPolicy` (the
  generalisation of today's `CallerPolicy`), and a set of
  trait-shaped handlers (`CStoreHandler`, `CFindHandler`,
  `CMoveHandler`, `CEchoHandler`).
- `crates/study-pipeline` — Per-profile worker pool. The 10-second
  debounce, the bounded `mpsc` queue, the N upload-worker tasks.
  Takes a `Shipper` callback (`async fn ship(study_uid, &Path)
  -> Result<()>`) and a study directory provider. Also hosts the
  pure backoff arithmetic (`RetryPolicy`, `next_retry_delay`) moved
  from `transmitter/backoff.rs`.
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

        /// Disk-space thresholds for the (shared) storage volume.
        /// Global, not per-profile: the volume is shared, so the
        /// lowest threshold would win anyway. Defaults match 1.9
        /// behaviour: warn below 2048 MB, pause uploads below 512 MB.
        #[serde(default)]
        pub disk: DiskConfig,

        /// One or more profiles. Must be non-empty. Each profile's
        /// `id` must be unique. No two profiles may bind the same
        /// `(local.ip, local.port)` pair.
        pub profiles: Vec<ProfileConfig>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DiskConfig {
        #[serde(default = "default_disk_warn_mb")]     // 2048
        pub warn_mb: u64,
        #[serde(default = "default_disk_critical_mb")] // 512
        pub critical_mb: u64,
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

        /// Accept associations from ANY calling AE title and store
        /// received instances locally WITHOUT queueing them for
        /// upload. Diagnostic use only; defaults to false. When
        /// false, the profile's SCP only admits calling AE titles
        /// present in this profile's cached Aurabox services
        /// catalog (AURA-2296 behaviour, now per profile).
        #[serde(default)]
        pub permissive_mode: bool,

        /// Upload retry policy. Defaults match 1.9 behaviour.
        #[serde(default)]
        pub retry: RetryConfig,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RetryConfig {
        /// Attempts before a study is marked terminally FAILED.
        #[serde(default = "default_max_attempts")]   // 10
        pub max_attempts: u32,
        /// Base delay (seconds) before the first retry; grows
        /// exponentially per attempt.
        #[serde(default = "default_retry_base")]     // 30
        pub base_seconds: u64,
        /// Cap (seconds) the exponential backoff is clamped to.
        #[serde(default = "default_retry_cap")]      // 3600
        pub cap_seconds: u64,
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
When the profile has `permissive_mode: true`, the C-STORE handler
stores the instance but skips `pipeline.enqueue` — the existing 1.9
behaviour, now per profile.

Association acceptance: the existing `CallerPolicy` (AURA-2296)
generalises into an `AccessPolicy` trait on `GatewayConfig` in
`dimse-gateway` — the same seam as `dicom_ul`'s `AccessControl`
trait, which the current code already implements. `bounce-aurabox`
provides the concrete policy: admit a calling AE title only if it
appears in this profile's cached services catalog, or admit anyone
when the profile is permissive. The services catalog cache moves
from the Tauri plugin-store (`store/pacs_cache.rs`) into a new
`pacs_services` table in `bounce.db`, keyed by `profile_id`, written
by each profile's poller on its existing refresh cadence and on the
explicit "Refresh" action in the UI.

The shutdown channel becomes a `tokio::sync::broadcast::Sender<()>`
held by a new `ProfileRuntime` struct (see Phase 5) so that the
receiver, the workers, the retry scheduler, and the poller can be
cancelled in lockstep.

The database migration for profiles has two parts. The simple part
is additive:

    ALTER TABLE upload_attempts ADD COLUMN profile_id TEXT NOT NULL DEFAULT 'default';
    CREATE TABLE IF NOT EXISTS pacs_services (
        profile_id TEXT NOT NULL,
        service_id TEXT NOT NULL,
        ae_title   TEXT NOT NULL,
        host       TEXT NOT NULL,
        port       INTEGER NOT NULL,
        name       TEXT,
        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (profile_id, service_id)
    );

The hard part is the `studies` table: its uniqueness must change
from `study_uid` alone to `(profile_id, study_uid)`, because the
same study can legitimately arrive on two profiles. SQLite cannot
alter constraints in place, so the migration rebuilds the table:
create `studies_new` with the composite UNIQUE constraint and a
`profile_id TEXT NOT NULL DEFAULT 'default'` column, copy all rows,
drop `studies`, rename, and recreate indexes — all inside one
transaction, guarded by a check that the column does not already
exist (matching the `add_column_if_missing` idiom already used in
`src-tauri/src/db/migrations.rs`). Existing rows get
`profile_id = 'default'` so older databases continue to round-trip;
the upgraded binary maps the old single-config install onto one
"default" profile on first boot (see Phase 9 / Tauri port).

This goes into the migrations module at
`crates/bounce-aurabox/src/db/migrations.rs`.

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

`study-pipeline` also absorbs the pure backoff math from
`transmitter/backoff.rs` as a `backoff` module (`RetryPolicy`,
`next_retry_delay`) — it is already free of I/O and Tauri, and any
"Bounce for X" needs the same capped-exponential arithmetic.

`bounce-aurabox` defines an `AuraboxShipper` (implements `Shipper`)
that preserves the 1.6 upload lifecycle for one study end-to-end:

1. Check the global disk guard (`DiskConfig` thresholds); if free
   space is critical, leave the study QUEUED and return without
   attempting (the retry scheduler re-dispatches it later).
2. Atomically claim the study via
   `Database::claim_study_for_upload(profile_id, study_uid, …)` —
   eligible states IN_PROGRESS / QUEUED / RETRYING; a study already
   claimed or terminal is skipped. This is also what makes worker
   dispatch idempotent: the same study UID can sit in the queue
   twice (debounce + retry scheduler racing) and only one attempt
   runs.
3. Run compress → `upload_init` → TUS upload → `upload_save` →
   cleanup, then `mark_attempt_success`.
4. On failure, `mark_upload_failed` with a `next_retry_at` computed
   from this profile's `RetryConfig` via
   `study_pipeline::backoff::next_retry_delay`; a study past
   `retry.max_attempts` goes terminally FAILED. Either way an
   `upload_attempts` audit row is written (the Transactions view is
   built on these) and a `transactions-updated` event is emitted via
   the `EventSink`.

The receiver enqueues a study UID via `pipeline.enqueue(study_uid)`
after the debounce window — except in permissive mode, where nothing
is enqueued. The per-profile **retry scheduler** (today's
`transmitter/retry_scheduler.rs`, kept in `bounce-aurabox`) scans
every 30 seconds for studies of this profile due for an attempt
(`find_due_for_retry` scoped by `profile_id`) and enqueues them into
the same pipeline, so workers are the single execution path for all
uploads — debounced, retried, and manual alike. The process-global
static `UPLOAD_SEMAPHORE` (`MAX_CONCURRENT_UPLOADS = 2`) is deleted;
the profile's `workers` count is the concurrency limit.

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
(with the three Aurabox-aware handlers and the per-profile
`AccessPolicy`), the `study-pipeline` worker pool, the per-profile
retry scheduler, and the per-profile Aurabox poller
(`aurabox-client` calling `fetch_pending_jobs` /
`fetch_pending_queries` on this profile's API key every
`aurabox.poll_interval_secs`). All spawned tasks subscribe to the
broadcast shutdown channel.

The Aurabox-specific `MAX_CONCURRENT_SENDS` constant in
`query/poller.rs` becomes the per-profile
`max_concurrent_sends` field (default 2) and is plumbed through.

The Tauri commands `retry_study` and `cancel_retry` are reworked to
resolve the study's profile from its `profile_id` column and then
enqueue into (or mark STOPPED in) that profile's pipeline; today
they construct a `Transmission` ad hoc.

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
  chunk size, poll interval, delete-after-success toggle, the
  permissive-mode toggle (with its existing destructive-warning
  banner, now scoped to the profile row), and the retry policy
  fields. Global (non-profile) settings — disk thresholds, telemetry
  / send-logs, `start_receiver_on_start`, start-on-login, automatic
  updates — render in a separate "Application" section. Validation
  runs client-side and is re-checked by the backend before save.
- Save action calls a new IPC command, `save_config(BounceConfig)`,
  which writes `bounce.json` atomically (write-to-temp + rename) and
  then triggers `Runtime::reload()` on the backend. The runtime
  supports reloading by stopping any profiles whose binding has
  changed, starting any new ones, and leaving alone any profiles
  whose `local` and `aurabox` blocks are unchanged so the worker
  pool and any in-flight uploads survive. This generalises the 1.9
  behaviour where saving receiver-affecting settings restarts the
  receiver automatically.
- "Studies", "Transactions", and "Logs" pages get a profile filter
  dropdown driven by the `profile_id` column in the database. The
  Transactions view's Retry and Stop buttons keep working; they now
  route through the owning profile's pipeline (see Phase 5).
- The dashboard status summary aggregates across profiles, with a
  per-profile breakdown row per profile.

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
plan), recent commits including the 1.9.1 release tag, Cargo
≥ 1.80 (matching `rust-version = "1.80"` in `src-tauri/Cargo.toml`).
If the version has moved past 1.9.1, re-verify the Context and
Orientation section against the code before starting — that section
was last verified against 1.9.1 (2026-06-10).

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

Add `bounce_aurabox::config::{BounceConfig, ProfileConfig, ...}` with
`serde` derive and a `validate()` method. Add round-trip tests that
load `crates/bounce-cli/examples/bounce.example.json` and assert the
expected profile count, IDs, and validation results. Sample contents:

    {
      "version": "2",
      "storage_dir": "/var/lib/bounce",
      "telemetry": null,
      "disk": { "warn_mb": 2048, "critical_mb": 512 },
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
          "delete_after_success": true,
          "permissive_mode": false,
          "retry": { "max_attempts": 10, "base_seconds": 30, "cap_seconds": 3600 }
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
          "delete_after_success": false,
          "permissive_mode": false,
          "retry": { "max_attempts": 5, "base_seconds": 60, "cap_seconds": 1800 }
        }
      ]
    }

### Step 6: Move DIMSE wire code into `dimse-gateway`; add `profile_id` columns (M4)

Lift the accept loop, PDU parsing, and ServerAssociationOptions
construction from `bounce-aurabox/src/receiver/dicom_server.rs`
into `crates/dimse-gateway/src/`, including the `AccessPolicy` hook
(generalising the existing `CallerPolicy` / `AccessControl`
implementation — its unit tests move with it). Define the
`CStoreHandler`, `CFindHandler`, `CMoveHandler`, `CEchoHandler`
traits. Implement them concretely in
`bounce-aurabox::receiver::handlers` against the existing
Aurabox-aware behaviour, including the permissive-mode store-only
path. Write the SQLite migrations described in Phase 4: the
`studies` table rebuild for the composite `(profile_id, study_uid)`
uniqueness, the additive `upload_attempts.profile_id` column, and
the new `pacs_services` table that replaces the plugin-store PACS
cache. Confirm:

    cd src-tauri && cargo test -p dimse-gateway
    cd src-tauri && cargo test -p bounce-aurabox receiver
    cd src-tauri && cargo test -p bounce-aurabox migrations

Receiver and migration tests pass with the new shape, including a
test that migrates a pre-2.0 database fixture (with populated
`studies` and `upload_attempts` rows) and asserts no rows are lost
and all rows carry `profile_id = 'default'`. Legacy single-profile
behaviour is reproduced exactly when `profiles` has length 1.

### Step 7: Move the worker pool into `study-pipeline` (M5)

Lift the debounce + worker shape from
`bounce-aurabox/src/transmitter/transmission.rs` into
`crates/study-pipeline/src/`, and move the pure backoff functions
from `bounce-aurabox/src/transmitter/backoff.rs` into
`study_pipeline::backoff` (their unit tests move with them). Define
the `Shipper` trait. Write `AuraboxShipper` in
`bounce-aurabox/src/transmitter/`, preserving the claim → attempt →
record lifecycle from today's `attempt_upload` (see Phase 5). Delete
the `UPLOAD_SEMAPHORE` static. Rework the retry scheduler to scan
per profile and enqueue into the profile's pipeline. Verify with a
new integration test in `crates/study-pipeline/tests/worker_pool.rs`
that two studies pushed back-to-back into a 2-worker pool are
processed concurrently (measured by start/finish timestamps in a stub
`Shipper` that sleeps for 500 ms). The test must fail if work is
serialised. The same crate's `examples/in_memory_shipper.rs` is the
M9 vendor-neutrality probe. Also verify in `bounce-aurabox` that the
existing retry tests still pass: a failing shipper produces an
`upload_attempts` row, sets `next_retry_at` per the profile's
`RetryConfig`, and goes terminally FAILED after `max_attempts`.

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

- `cd src-tauri && cargo build` builds all seven workspace members.
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
  gateway-handler path passes with no behavioural regression,
  including the `CallerPolicy`/`AccessPolicy` tests (accept-any
  admits everyone; an empty registered list rejects everyone; a
  registered caller is admitted; an unregistered caller is rejected
  even when others are registered).
- A fresh `bounce.db` written by the new code shows a `profile_id`
  column on `studies` and `upload_attempts`, a composite UNIQUE on
  `studies(profile_id, study_uid)`, and the new `pacs_services`
  table (`sqlite3 bounce.db ".schema studies"` /
  `".schema pacs_services"`).
- The pre-2.0 fixture migration test passes: row counts are
  unchanged across the `studies` table rebuild, and every migrated
  row carries `profile_id = 'default'`.
- Two profiles can store the *same* study UID concurrently and both
  rows exist, distinguished by `profile_id` (regression test for the
  composite-key change).

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
- Database migrations are guarded so re-running them is a no-op: the
  additive ones use the existing `add_column_if_missing` /
  `CREATE TABLE IF NOT EXISTS` idiom, and the `studies` table rebuild
  (needed for the composite `(profile_id, study_uid)` uniqueness)
  first checks whether `profile_id` already exists on `studies` and
  skips entirely if it does. The rebuild runs inside a single
  transaction, so a crash mid-migration rolls back to the pre-2.0
  schema rather than leaving a half-renamed table.
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

    /// Decides whether to admit an incoming association. Generalises
    /// the existing `CallerPolicy` (AURA-2296); product crates supply
    /// an implementation that consults their own registry. The
    /// built-in `AcceptAny` admits everyone (used by permissive mode
    /// and the echo_only example).
    pub trait AccessPolicy: Send + Sync + 'static {
        fn admit(&self, calling_ae_title: &str, called_ae_title: &str)
            -> Result<(), RejectReason>;
    }

    pub struct AcceptAny;

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

In `crates/study-pipeline/src/backoff.rs` (moved verbatim from
`src-tauri/src/transmitter/backoff.rs`, tests included):

    pub struct RetryPolicy {
        pub max_attempts: u32,
        pub base_delay: Duration,
        pub cap: Duration,
    }

    impl RetryPolicy {
        pub fn should_retry(&self, attempt_no: u32) -> bool;
    }

    pub fn next_retry_delay(attempt_no: u32, policy: &RetryPolicy)
        -> Duration;

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
- `fs2` (already a dependency, used by the disk guard) moves with
  the disk-check helper into `bounce-aurabox`.

Crates removed from `bounce-aurabox` (still used by `bounce-tauri`):

- `tauri`, `tauri-plugin-store`, `tauri-plugin-log`,
  `tauri-plugin-process`, `tauri-plugin-shell`, `tauri-plugin-dialog`,
  `tauri-plugin-opener`, `tauri-plugin-os`, `tauri-plugin-updater`,
  `tauri-plugin-single-instance`, `tauri-plugin-autostart`.

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

- 2026-06-10 — Re-baselined against the current codebase. The plan
  was drafted against ~1.5.0; the repo is now at 1.9.1 and several
  subsystems landed in between that the plan must preserve rather
  than accidentally regress:
  - **Upload retry/recovery (1.6):** status machine on `studies`
    (`attempts`, `next_retry_at`, `last_error`), per-attempt
    `upload_attempts` audit table, pure backoff module
    (`transmitter/backoff.rs`), a 30-second DB-scanning retry
    scheduler, a process-global two-permit upload semaphore, and a
    disk-space guard (`store/disk.rs`, `fs2`) with warn/critical
    thresholds. The plan now: moves the pure backoff math into
    `study-pipeline`; keeps the retry scheduler and disk guard in
    `bounce-aurabox` (per-profile scheduler, global disk
    thresholds); folds the claim/attempt/record lifecycle into
    `AuraboxShipper`; and deletes the global semaphore in favour of
    per-profile worker counts. Retry policy becomes a per-profile
    `retry` block in the schema; disk thresholds a global `disk`
    block.
  - **Transactions view (1.7):** new commands (`retry_study`,
    `cancel_retry`, `study_upload_attempts`,
    `current_upload_attempts`, `dashboard_stats`, bulk operations)
    and `transactions-updated` events. The UI-port phase now scopes
    the Transactions view per profile and routes Retry/Stop through
    the owning profile's pipeline.
  - **AE-title allow-listing (AURA-2296, 1.7):** the receiver now
    rejects callers not present in the cached Aurabox services
    catalog via a `CallerPolicy` implementing `dicom_ul`'s
    `AccessControl`. The plan generalises this as an `AccessPolicy`
    hook on `dimse-gateway` and makes the allow-list per-profile,
    moving the PACS cache from the plugin-store into a
    `pacs_services` DB table.
  - **Permissive mode (1.9):** per the new `permissive_mode` setting
    the SCP accepts any caller and stores without queueing uploads;
    now a per-profile boolean in the schema.
  - **Lifecycle hardening (1.8–1.9.1):** graceful receiver shutdown,
    `start_receiver_on_start`, tray/close-to-tray fixes,
    single-instance plugin, autostart plugin, automatic updates.
    The plan records that single-instance stays Tauri-side and
    `bounced` instead fails fast on port-bind conflict.
  - The `studies` uniqueness constraint must become composite
    `(profile_id, study_uid)` — the same study UID can arrive on two
    profiles — which makes the migration a guarded table rebuild
    rather than a simple `ADD COLUMN`; the earlier decision-log
    entry assuming additive-only migration is superseded in place.
  Context and Orientation was rewritten against 1.9.1, the schema
  and sample JSON gained `permissive_mode`/`retry`/`disk`, and the
  acceptance criteria gained migration-fixture and same-study-UID
  regression tests.
