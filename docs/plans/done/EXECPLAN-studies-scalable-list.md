# Scale the Studies view to thousands of items: table layout, server-side search, bulk actions, delete-all

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

This repository follows the ExecPlan conventions described in `.claude/skills/codex-plans/SKILL.md`. There is no `PLANS.md` checked in at the repository root; the skill file is the source of truth for plan structure and lifecycle. This document must be maintained accordingly: append to `Progress` at every stopping point, record every design change in `Decision Log`, and write an `Outcomes & Retrospective` entry at completion.

## Purpose / Big Picture

Today the Studies page in the Bounce desktop app (the screen reachable from the left sidebar "Studies" entry) shows each received DICOM study as a full-width card. A typical Bounce install behind a busy modality may accumulate hundreds — eventually thousands — of studies. The current card layout consumes roughly 96 pixels of vertical space per study, the only per-row action is "Send study" plus a kebab "Delete", and although the Rust backend supports `LIMIT/OFFSET` pagination, the React page miscalculates totals from `studies.length` (which is only the current page) so the page-number row often shows "1" even when there are many pages of data. There is no way to search, no way to act on more than one study at a time, and no way to wipe the local store from the UI.

After this change is implemented, a Bounce operator can:

1. See twenty-five studies per screen in a compact table (description, patient, status, created, UID, actions) instead of three or four cards.
2. Type into a search box at the top of the page and have the visible list narrow to studies whose description, patient name, patient ID, accession number, or Study UID contain the typed text. The search runs on the SQLite backend so it works against the full dataset, not just the current page.
3. Tick a checkbox at the start of any row (or a master checkbox in the table header to select every row on the current page), then click "Send selected" or "Delete selected" in an action bar that appears above the table. Both bulk operations show a confirmation dialog before running, and after they complete the table refreshes.
4. Click a "Delete all studies" button in the table toolbar, confirm a strongly-worded prompt, and remove every study from the database and from local on-disk storage.
5. Change page size between 25 / 50 / 100 via a dropdown next to the pagination controls. The pagination footer reports "Showing X to Y of Z studies" using the true total reported by the backend.

How to see it working: run `make dev`, ingest a handful of studies via `make dicom-send FILE=path.dcm` (or paste several DICOM files through the receiver), navigate to the Studies tab, and verify each item in the list above with the steps in `Validation and Acceptance`.

## Progress

- [x] Milestone 1 — Backend: add server-side search filter, bulk-send, bulk-delete, and delete-all-studies commands.
- [x] Milestone 2 — Frontend foundation: lift pagination metadata into Redux, add `search` and `limit` parameters end-to-end, debounce the search input.
- [x] Milestone 3 — Frontend layout: replace the card list with a compact `<table>` and extract it into a dedicated component.
- [x] Milestone 4 — Frontend interaction: per-row + select-all-on-page checkboxes, sticky action bar, bulk Send / bulk Delete with confirmation.
- [~] Milestone 5 — Frontend toolbar: "Delete all studies" button with confirmation, page-size selector, polish complete; manual end-to-end verification remains (deferred to an operator with a running Bounce instance + DCMTK or Orthanc; the developer machine running this implementation has no DICOM peer to exercise the live flows).

(Append timestamped entries as work proceeds. When stopping mid-milestone, split the bullet into "completed: ..." and "remaining: ...".)

- 2026-05-28 — Milestone 1 completed.
  - Added `Database::get_studies_paginated_filtered` (LIKE search across `study_description`, `patient_name`, `patient_id`, `accession_no`, `study_uid`) and made `get_studies_paginated` a thin delegate.
  - Extended `Database::current_studies` to accept `search: Option<String>`, trimming whitespace and embedding `"search"` into the returned pagination JSON.
  - Added Tauri commands `bulk_send_studies`, `bulk_delete_studies`, `delete_all_studies`; widened the `current_studies` command signature to accept `search`; registered all three new commands in `generate_handler!`.
  - Added `test_get_studies_paginated_filtered` covering description, patient name, accession, UID, whitespace-only, `None`, and no-match cases.
  - `cargo build`, `cargo test` (227 passed), `cargo clippy --all-targets` clean on changed files (two pre-existing warnings in `aura/query_api_tests.rs` and `query/cfind_tests.rs` remain — unrelated).

- 2026-05-28 — Milestones 2–5 implementation completed; manual end-to-end verification deferred.
  - `app/lib/types.ts`: added `Pagination` interface, widened `Study` with optional `patient_name`, `patient_id`, `accession_no`, `series_count`, `images`, `created_at`, `updated_at`, `sent_at`. Tightened `exists` from `string` to `boolean` (the backend already emits a bool — the previous typing was wrong; only consumer was `study.exists && ...` which works for both).
  - `app/lib/store.ts`: added `pagination: Pagination | null` to `State`, `setCurrentStudies` now stores both `studies` and `pagination`.
  - `app/components/EventHandler.tsx`: no change required — already dispatches the full event payload.
  - `app/components/ui/checkbox.tsx` (new): minimal checkbox primitive with `indeterminate` support; uses `useImperativeHandle` to forward the inner ref while also setting `.indeterminate` in an effect.
  - `app/components/StudiesTable.tsx` (new): description, patient (with patient_id beneath), status badge, relative-time "Received" with absolute time in `title`, last-12-chars UID with full UID in `title`, ghost Send (when `exists`) + ghost Delete in the actions column. Master checkbox in the header with indeterminate state. Local relative-time formatter — no `date-fns` dependency added.
  - `app/studies/page.tsx` (rewrite): replaced the card list with `<StudiesTable>`. Added search input with 300 ms debounce, page-size selector (25/50/100), "Delete all" button, action bar appearing when rows are selected. All confirmations use `confirm` from `@tauri-apps/plugin-dialog`. Selection state lives in the page component, keyed by `study_uid`, and is cleared on page/pageSize/search change.
  - Verified: `npm run build` succeeds with no new TypeScript errors. `npm run lint` shows no new warnings on changed files (the five pre-existing `react-hooks/exhaustive-deps` warnings on `EventHandler`/`PacsServices`/`Settings`/`Tray` are untouched).
  - Verified: `cargo test` continues to pass.
  - Not done: manual end-to-end verification (`Validation and Acceptance` items 1–12) — requires a running Bounce instance with several ingested studies. Listed as a follow-up for whoever runs the app next.

## Surprises & Discoveries

(Empty until implementation begins. Add observations with short evidence — test output, log snippets, screenshots paths.)

## Decision Log

- Decision: Server-side `search` parameter is a single free-text string, matched with `LIKE '%' || ? || '%'` against five columns (`study_description`, `patient_name`, `patient_id`, `accession_no`, `study_uid`) joined by `OR`.
  Rationale: Operators searching for a study know one of: the patient's name, the patient's hospital ID, the accession the technologist gave them, or part of the description ("US Ankle"). UIDs are occasionally pasted from another system. A single input is easier to use than per-column filters, and at the expected scale (low thousands of rows) `LIKE` without a full-text index is fast enough. `LIKE` is case-insensitive in SQLite by default for ASCII; we do not need `LOWER()`.
  Date/Author: 2026-05-28 / planning.

- Decision: Selection state lives in the Studies page component, keyed by `study_uid`, and is cleared whenever the page, the search query, or the page size changes.
  Rationale: Cross-page bulk operations would be confusing ("did I select 47 items or just the 12 on this page?"). Clearing on filter change matches Gmail's behaviour and prevents acting on rows the user can no longer see. Local component state (not Redux) is correct because the selection has no consumers outside this page.
  Date/Author: 2026-05-28 / planning.

- Decision: Use the existing `@tauri-apps/plugin-dialog` (already a registered plugin per `src-tauri/src/main.rs` line 321 and a dependency aligned in commit 279bc7d) for all confirmations rather than adding an in-app modal component.
  Rationale: The project has no `Dialog`/`AlertDialog` UI primitive today. Introducing one is out of scope for this work. The OS-native confirm dialog is unambiguous, accessible, and already supported. Wording is chosen per call site to make consequences explicit.
  Date/Author: 2026-05-28 / planning.

- Decision: Bulk Send and Bulk Delete are implemented as new dedicated Tauri commands (`bulk_send_studies`, `bulk_delete_studies`) that take `Vec<String>` of UIDs and iterate, rather than the frontend awaiting `send_study` / `delete_study` in a loop.
  Rationale: One IPC round trip instead of N keeps the UI responsive when a user selects fifty studies. The Rust side can also batch the final UI-refresh emit, so the list does not re-render after every per-item delete.
  Date/Author: 2026-05-28 / planning.

- Decision: "Delete all studies" is a brand-new Tauri command (`delete_all_studies`) — not a reuse of `reset_app`. `reset_app` was designed for factory reset and may grow other side effects (clearing settings, API keys); the Studies tab needs only studies+files cleared.
  Rationale: Single responsibility. Future maintainers can extend `reset_app` without accidentally widening the blast radius of the Studies "Delete all" button.
  Date/Author: 2026-05-28 / planning.

- Decision: Out of scope for this plan: server-side sorting (the existing `ORDER BY created_at DESC` is fine), per-column filters, virtualised scrolling, live auto-refresh on `study-received` events, and an in-app modal component.
  Rationale: Each is a separate piece of work with its own UX questions; the user's request was scoped to compact table, pagination (already partly there — fix), search, bulk send/delete, and delete-all. Note in the retrospective if any of these become obviously necessary during testing.
  Date/Author: 2026-05-28 / planning.

## Outcomes & Retrospective

### 2026-05-28 — Implementation complete, manual verification outstanding

**What now works that did not before:**

1. **Pagination totals are correct.** The footer reads from the backend's `pagination.total_items` rather than the length of the current page, so it now shows "Showing 1 to 25 of N" where N is the true total.
2. **Server-side search.** A debounced search input at the top of the page filters across description, patient name, patient id, accession number, and Study UID. It runs on SQLite, so it works against the full dataset, not just the current page.
3. **Compact table layout.** Studies are listed in a `<table>` with 7 columns at roughly 40 px per row; 25 rows fit on the default window size.
4. **Bulk Send and Bulk Delete.** Per-row + master checkboxes with indeterminate state; an action bar appears when at least one row is selected; both actions show an OS confirmation dialog and refresh the page on completion.
5. **Delete all studies.** A destructive toolbar button (hidden when the table is empty) that confirms then clears both the SQLite rows and the on-disk study directory.
6. **Page-size selector.** 25 / 50 / 100, with the page reset to 1 on change.

**What was deferred:**

- Manual end-to-end verification per `Validation and Acceptance` items 1–12, density measurement (item 11), and the cross-tab non-regression check (item 12). These need a running Bounce instance with ingested studies; the implementation environment does not have one.
- Out-of-scope items called out in the Decision Log remain out of scope: server-side sorting, per-column filters, virtualised scrolling, live auto-refresh on `study-received` events, in-app modal component.

**What the next person to touch this page should know:**

- The `current_studies` Tauri command now takes an optional `search` argument. The frontend always passes `null` when the search box is empty; the backend trims whitespace and treats whitespace-only as no filter.
- `Study.exists` in TypeScript was previously typed as `string` but the backend has always emitted a bool — the type was wrong, not the data. Only `study.exists && ...` consumed it, so flipping the type is a no-op at runtime.
- Selection state is intentionally per-page (cleared when page/pageSize/search changes). If you ever want cross-page selection, surface the selection size in the action bar prominently and rethink how the "select all on page" checkbox interacts with selections off-page.
- The new bulk and delete-all commands log per-item failures but do not return them to the UI. If a richer UX is needed, the commands should return a `{ok: [...], failed: [{uid, error}]}` shape and the page should render a toast.
- The page reuses the existing OS-native confirm dialog from `@tauri-apps/plugin-dialog`. If an in-app modal component is ever introduced, swap these call sites first — they're the highest-value places for richer confirmation copy.

## Context and Orientation

Bounce is a cross-platform desktop app built with Tauri 2.x. The Rust backend lives in `src-tauri/`, the React/Next.js 15 frontend lives in `app/`. Communication is via Tauri's IPC: the frontend calls `invoke('<command_name>', args)` and the backend may push updates by calling `app.emit('<event_name>', payload)`, which the frontend receives via `useTauriEvent` or the central `EventHandler` component.

A "study" in this project is a DICOM imaging study — a collection of image series belonging to one patient examination. Studies are received over the network by the C-STORE service (medical-imaging file push), persisted to SQLite (`src-tauri/src/db/`) plus a directory on disk under the Bounce data folder, and can be uploaded to the Aurabox cloud via TUS (a resumable HTTP upload protocol).

Key files for this work, all paths repository-relative:

- `app/studies/page.tsx` — the page component that renders today's card list, owns pagination state, and calls `invoke('current_studies', { page, limit })`. This is the primary file the refactor edits.
- `app/lib/store.ts` — the Redux Toolkit store. The `mainSlice.setCurrentStudies` reducer currently throws away pagination metadata from the backend response and stores only `studies`. This will be widened.
- `app/lib/types.ts` — TypeScript types for `Study` and `CurrentStudies`. Will be extended with the pagination fields the backend already returns, plus optional `patient_name` / `patient_id`.
- `app/components/EventHandler.tsx` — listens for the `current-studies` Tauri event (and others) and dispatches into Redux. The dispatch will need to forward pagination data, not just `studies`.
- `app/components/ui/button.tsx`, `app/components/ui/card.tsx`, `app/components/ui/badge.tsx`, `app/components/ui/input.tsx` — existing shadcn-style UI primitives we reuse. No table or checkbox primitive exists today; we add a minimal `Checkbox` and build the table from semantic HTML.
- `app/lib/helpers.ts` — utility module; already exports `classNames` and `formatDicomDateAndTime`. No new helpers required beyond a small debounce hook (added inline in the page) and a relative-time formatter.
- `src-tauri/src/main.rs` — registers Tauri commands at the `tauri::generate_handler!` call near the bottom of the file (line 443). New commands are added both as `#[tauri::command] async fn` definitions and as entries in this macro.
- `src-tauri/src/db/database.rs` — owns the SQLite layer via `sqlx`. `current_studies(page, limit) -> Value` (line 125) builds the paginated JSON response. `get_studies_paginated(offset, limit)` (line 221) runs the underlying `SELECT`. `delete_study(study_uid)` (line 268) and `clear_studies()` (line 283) already exist.
- `src-tauri/src/db/models.rs` — the `Study` struct (line 6). No schema change needed; we only read existing columns.
- `src-tauri/src/transmitter/` — the `Transmission` type whose `delete_study`, `delete_local_study_meta`, `clear_storage`, and `send_study` methods are called from the existing per-item Tauri commands in `src-tauri/src/main.rs`. We reuse them.

Definitions used in this plan:

- **Tauri command** — a Rust async function annotated with `#[tauri::command]` and registered in the `generate_handler!` macro. It becomes callable from the frontend with `invoke('function_name', { camelCaseArg: value })`. Note: argument names are passed camelCase from JS, snake_case in Rust — Tauri converts automatically.
- **Study UID** — the DICOM Study Instance UID, a string like `1.3.6.1.4.1.5962.99.1.2786334768...` that uniquely identifies a study. Case-sensitive, never normalised. Used as the primary handle in every operation.
- **TUS** — a resumable HTTP upload protocol. Relevant here only because `Transmission::send_study` uses it; we never need to think about chunks in this plan.
- **C-STORE / C-FIND** — DICOM network operations for pushing and querying studies, respectively. Relevant here only because they are the source of the studies we are now listing.
- **Redux slice** — a Redux Toolkit term for a co-located reducer, actions, and initial state for one section of the store. The Bounce app has a single `mainSlice` in `app/lib/store.ts`.

## Plan of Work

The work splits into five milestones. Each is independently mergeable: at the end of any milestone the app still builds, the existing behaviour still works, and the new behaviour up to that point is observable.

### Milestone 1 — Backend: search, bulk, delete-all

Extend `src-tauri/src/db/database.rs`:

1. Add `pub async fn get_studies_paginated_filtered(&self, offset: i64, limit: i64, search: Option<&str>) -> Result<(Vec<Study>, i64)>`. When `search` is `None` or whitespace-only the body is identical to the existing `get_studies_paginated`. When a search string is present, both the `SELECT` and the `SELECT COUNT(*)` queries must include `WHERE study_description LIKE ?1 OR patient_name LIKE ?1 OR patient_id LIKE ?1 OR accession_no LIKE ?1 OR study_uid LIKE ?1`, with `?1` bound to `format!("%{}%", trimmed_search)`. Use the same bound value for both queries so the count matches the page. Keep `ORDER BY created_at DESC LIMIT ? OFFSET ?` in the SELECT.
2. Change `pub async fn current_studies(&self, page: u32, limit: u32)` to `pub async fn current_studies(&self, page: u32, limit: u32, search: Option<String>)` and route through the new filtered function. Embed the trimmed `search` string into the returned pagination JSON object as `"search": <string or null>` so the frontend can confirm round-trip.
3. The existing `get_studies_paginated` stays (or is reimplemented as a thin call to the filtered version with `None`) so any other caller is unaffected.

Edit `src-tauri/src/main.rs`:

4. Change the `current_studies` Tauri command signature to:

        #[tauri::command]
        async fn current_studies(
            app: AppHandle,
            page: Option<u32>,
            limit: Option<u32>,
            search: Option<String>,
        ) -> Result<(), String>

   forward `search` to `database.current_studies(...)`. The behaviour when `search` is absent must be identical to today.

5. Add three new Tauri commands. Place them near `send_study` and `delete_study` for discoverability:

        #[tauri::command]
        async fn bulk_send_studies(app: AppHandle, study_uids: Vec<String>) -> Result<(), String> {
            let transmission = Transmission::new(app);
            for uid in study_uids {
                if let Err(e) = transmission.send_study(uid.clone()).await {
                    log_error!("bulk_send_studies: failed for {}: {}", uid, e);
                }
            }
            Ok(())
        }

        #[tauri::command]
        async fn bulk_delete_studies(app: AppHandle, study_uids: Vec<String>) -> Result<(), String> {
            let transmission = Transmission::new(app.clone());
            let database = app.state::<Database>();
            for uid in study_uids {
                let _ = transmission.delete_study(uid.clone()).await;
                let _ = transmission.delete_local_study_meta(uid.clone()).await;
                let _ = database.delete_study(uid.clone()).await;
            }
            Ok(())
        }

        #[tauri::command]
        async fn delete_all_studies(app: AppHandle) -> Result<(), String> {
            let database = app.state::<Database>();
            let transmission = Transmission::new(app);
            let count = database.clear_studies().await.map_err(|e| e.to_string())?;
            transmission.clear_storage().await.map_err(|e| e.to_string())?;
            log_info!("delete_all_studies: removed {} studies", count);
            Ok(())
        }

   The bulk commands deliberately do not abort on the first failure; medical-imaging operators expect a "best effort" semantics. Errors are logged and the operation continues. We do not (yet) return per-item results — `Validation and Acceptance` describes how to inspect the logs.

6. Register all three new commands in the `tauri::generate_handler![ ... ]` macro at line 443.

Tests:

7. Extend `src-tauri/src/db/database.rs` test module (or add a `#[cfg(test)] mod tests` if none exists for these functions): seed three studies with distinct descriptions and patient names into an in-memory SQLite, then assert that `get_studies_paginated_filtered(0, 10, Some("ankle"))` returns only the matching study and that the returned total count is 1.
8. Run `make test` (which proxies to `cargo test` in `src-tauri/`) and observe the new test passes.

### Milestone 2 — Frontend: pagination metadata in Redux, search + limit wiring

Edit `app/lib/types.ts`:

1. Add a `Pagination` interface:

        export interface Pagination {
            current_page: number,
            total_pages: number,
            total_items: number,
            limit: number,
            offset: number,
            has_next_page: boolean,
            has_previous_page: boolean,
            items_on_page: number,
            search: string | null,
        }

2. Extend `CurrentStudies` to include `pagination: Pagination`. Extend `Study` to include optional `patient_name?: string`, `patient_id?: string`, `accession_no?: string`, `sent_at?: string | null`, `series_count?: number`, `images?: number`. (The backend already includes them; we are just typing them.)

Edit `app/lib/store.ts`:

3. Add `pagination: Pagination | null` to `State`, default `null` in `initialState`. Replace `setCurrentStudies` body with:

        state.studies = action.payload.studies
        state.pagination = action.payload.pagination

   No existing consumer breaks because nothing else currently reads `pagination`.

Edit `app/components/EventHandler.tsx`:

4. No behaviour change required — it already dispatches the full payload from the `current-studies` event. Verify by reading the file.

Edit `app/studies/page.tsx`:

5. Read `pagination` from Redux (`useAppSelector((s) => s.main.pagination)`) and stop computing `totalItems` and `totalPages` from `studies.length`. Use `pagination?.total_items ?? 0`, `pagination?.total_pages ?? 0`, and derive `startIndex = pagination.offset` and `endIndex = pagination.offset + pagination.items_on_page` for the "Showing X to Y of Z" line.
6. Replace the constant `ITEMS_PER_PAGE = 10` with React state: `const [pageSize, setPageSize] = useState<number>(25)`. Set the initial fetch to 25.
7. Add `const [searchInput, setSearchInput] = useState<string>('')` and a `const [debouncedSearch, setDebouncedSearch] = useState<string>('')`. Implement a `useEffect` that updates `debouncedSearch` 300 ms after `searchInput` stops changing. (Plain `setTimeout` with cleanup — do not add a debounce dependency.)
8. Change `loadStudies` to accept `(page, limit, search)` and call `invoke('current_studies', { page, limit, search: search || null })`. Add `debouncedSearch` and `pageSize` to its dependency array and to the effect that calls it. When `debouncedSearch` changes, reset `currentPage` to 1 before the fetch.
9. Render an `Input` (from `@/app/components/ui/input`) at the top of the page bound to `searchInput`, with placeholder `"Search description, patient, accession, or UID…"`. Width `max-w-md`.

At the end of this milestone the page still looks like cards, but pagination numbers are correct and search works (you can verify by typing — items filter, the count updates, the page resets to 1).

### Milestone 3 — Frontend: table layout

Create `app/components/StudiesTable.tsx`. Props:

        interface StudiesTableProps {
            studies: Study[],
            selectedUids: Set<string>,
            onToggleSelect: (uid: string) => void,
            onToggleSelectAll: () => void,
            allOnPageSelected: boolean,
            someOnPageSelected: boolean,
            onSend: (study: Study) => void,
            onDelete: (study: Study) => void,
        }

Render a `<table className="w-full text-sm">` with a sticky `<thead>` and tailwind borders:

- Column 1: master checkbox (in `<th>`) and per-row checkbox (in `<td>`).
- Column 2: "Description" — `study.study_description`. If empty, show `"—"` in `text-muted-foreground`.
- Column 3: "Patient" — `study.patient_name || '—'`. Show patient_id underneath in `text-xs text-muted-foreground` when present.
- Column 4: "Status" — `Badge` with the existing `statusMap` variant lookup.
- Column 5: "Received" — formatted from `study.created_at` (this is an ISO string from the backend). Show relative time ("3 minutes ago") with the absolute time in a `title` attribute for hover. Implement the relative formatter as a small local helper at the top of the file; do not pull in `date-fns` unless it is already a dependency (check `package.json` first — add to `Decision Log` if you do).
- Column 6: "UID" — truncated to last 12 chars in a `<span className="font-mono text-xs">…{uid.slice(-12)}</span>`, with the full UID in `title`.
- Column 7: "Actions" — a small group: "Send" button (only when `study.exists`) and a "Delete" button. Use `Button variant="ghost" size="sm"` for both.

Row hover: `hover:bg-muted/40`. Row when selected: `bg-muted/60`. Header cells: `text-left px-3 py-2 font-medium text-muted-foreground`. Body cells: `px-3 py-2 align-top border-t`. Density target: 40 px per row.

Edit `app/studies/page.tsx`:

10. Remove the per-card JSX. In its place render `<StudiesTable ... />`, plumbing the selection state and the existing `sendStudy` / `deleteStudy` handlers through the props. The empty state (no rows) still renders the existing "No studies found" card — keep that, since a styled empty state in a table is awkward.

### Milestone 4 — Frontend: row selection + bulk action bar

Edit `app/studies/page.tsx`:

11. Add `const [selectedUids, setSelectedUids] = useState<Set<string>>(new Set())`.
12. Add a `useEffect` that clears `selectedUids` whenever `currentPage`, `pageSize`, or `debouncedSearch` changes. (See Decision Log.)
13. Implement `toggleSelect(uid)` and `toggleSelectAll()` (which selects all UIDs currently on the page if not all are selected; otherwise clears them).
14. Above the table, conditionally render an action bar when `selectedUids.size > 0`:

        <div className="flex items-center justify-between rounded-md border bg-muted/30 px-4 py-2">
            <span className="text-sm">{selectedUids.size} selected</span>
            <div className="flex gap-2">
                <Button size="sm" variant="outline" onClick={onBulkSend}>Send selected</Button>
                <Button size="sm" variant="destructive" onClick={onBulkDelete}>Delete selected</Button>
            </div>
        </div>

15. `onBulkSend`:

        const uids = Array.from(selectedUids)
        const ok = await confirm(
            `Send ${uids.length} studies to Aurabox?`,
            { title: 'Send selected studies', kind: 'info' }
        )
        if (!ok) return
        await invoke('bulk_send_studies', { studyUids: uids })
        setSelectedUids(new Set())
        await loadStudies(currentPage, pageSize, debouncedSearch)

    `confirm` is imported from `@tauri-apps/plugin-dialog`. Argument naming follows the existing Tauri JS-to-Rust convention (`studyUids` ↔ `study_uids`).

16. `onBulkDelete`: identical shape, calls `bulk_delete_studies`, dialog `kind: 'warning'`, and the message reads `"Delete ${uids.length} studies? This removes them from disk and cannot be undone."`

### Milestone 5 — Frontend: Delete-all, page-size selector, polish, end-to-end verification

Edit `app/studies/page.tsx` toolbar (the existing flex row that contains the Refresh button):

17. Left side now contains the search Input. Right side contains, in order: the page-size `<select>` (25 / 50 / 100), the Refresh button, a "Delete all" button (`variant="destructive"`, hidden when `pagination?.total_items === 0`).
18. "Delete all" handler:

        const total = pagination?.total_items ?? 0
        const ok = await confirm(
            `Delete all ${total} studies and their files? This cannot be undone.`,
            { title: 'Delete all studies', kind: 'warning' }
        )
        if (!ok) return
        await invoke('delete_all_studies')
        setSelectedUids(new Set())
        setCurrentPage(1)
        await loadStudies(1, pageSize, debouncedSearch)

19. Page-size `<select>` handler resets `currentPage` to 1 and refetches.

Polish:

20. Empty-search empty state (no rows, no search): the existing "No studies found" card.
21. Non-empty-search empty state (no rows match): a smaller inline message above the table reading `No studies match "<search>". Clear the search to see all studies.` with a "Clear" button that empties the input.
22. While `loadStudies` is in flight, the table is rendered with `opacity-60 pointer-events-none` so users see existing data dim rather than blanking the screen.

Manual end-to-end verification per `Validation and Acceptance`.

## Concrete Steps

All commands assume the working directory is the repository root: `/Users/xtfer/working/aurabx/_active/bounce`.

Milestone 1 — backend:

    # After editing src-tauri/src/db/database.rs and src-tauri/src/main.rs:
    cd src-tauri && cargo build
    cd .. && make test

    # Expected: cargo build succeeds with no warnings introduced by the new code;
    # `cargo test` finishes with "test result: ok. N passed; 0 failed".

Milestone 2 — frontend foundation. There is no separate type-check command; the next.js build does it:

    npm run build

    # Expected: Next.js build completes successfully. No TypeScript errors.

Milestones 3–5 — run the full app:

    make dev

    # In a second terminal, send a few test studies if you have DCMTK installed:
    make dicom-echo                              # confirms the receiver is up
    make dicom-send FILE=<path to .dcm file>     # repeat 5–10 times with
                                                 # different files to populate

    # Or start the bundled Orthanc test PACS:
    cd docker && docker-compose up -d
    # then push from Orthanc to BOUNCE@localhost:104 via its UI.

    # In the Bounce window, click "Studies" in the sidebar.

If `make dicom-send` is not available (no DCMTK installed), generate noise by re-pushing the same file many times to a fake/test Bounce instance, or seed the database directly via the SQLite CLI against the path in `~/.aurabox/bounce/` (look up the exact filename in `src-tauri/src/db/database.rs`). Note the chosen seeding method in `Surprises & Discoveries`.

After each milestone, commit. The commit message style established in this repo (per `git log`) is conventional commits — `feat:`, `fix:`, `refactor:`, etc.

## Validation and Acceptance

Run `make dev` and exercise the following scenarios. Each one is a pass/fail check; an implementation that fails any of these is not done.

1. **Pagination correctness.** With at least 30 studies in the database, the bottom of the page reads "Showing 1 to 25 of 30 studies" (not "Showing 1 to 25 of 25") and the pager shows two page buttons. Click page 2: the table shows the remaining 5 studies and the toolbar reads "Showing 26 to 30 of 30".

2. **Search.** Type a substring of one study's description into the search box. Within roughly half a second the table filters to matching rows only and the total updates ("Showing 1 to 1 of 1"). Clear the search: the full list returns and the page resets to 1.

3. **Search across columns.** Type a patient's last name → matching rows appear. Type a partial accession number → matching rows appear. Type the last 8 characters of a Study UID → exactly one row appears.

4. **Per-row actions still work.** Click the "Send" button on any row whose status badge is not already SENT and whose `exists` is true: confirm the existing send pipeline runs (status badge becomes SENT within a few seconds). Click "Delete" on a single row: the OS confirmation dialog appears; cancelling leaves the row in place; confirming removes it and the table refreshes.

5. **Bulk select within a page.** Tick the master checkbox in the header. All visible rows become checked and the action bar appears reading "25 selected". Untick the master checkbox: all clear. Tick three individual rows: the action bar reads "3 selected" and the master checkbox renders in indeterminate state.

6. **Selection clears on filter change.** Select three rows, then type into the search box. After the debounce fires, the action bar disappears and the table re-renders with no rows selected. Same when changing page or page size.

7. **Bulk Send.** Select five received-but-not-yet-sent studies. Click "Send selected". The dialog reads "Send 5 studies to Aurabox?". Confirm. Within a few seconds all five status badges flip to SENT (visible by refreshing or because the page refetches on completion).

8. **Bulk Delete.** Select five rows. Click "Delete selected". The dialog text mentions disk removal and irreversibility. Confirm. The five rows disappear from the table, the total count drops by five, the action bar disappears, and the on-disk study directories are gone (check `~/.aurabox/bounce/<studies-folder>/`).

9. **Delete all.** With a non-empty database, click "Delete all". The dialog includes the current count. Confirm. The table shows the empty-state card. The database and on-disk store are empty (`sqlite3 <db> "SELECT COUNT(*) FROM studies"` returns 0; the studies directory is empty).

10. **Page-size selector.** Switch from 25 to 100. The table refetches and shows up to 100 rows. The pagination footer reflects the new size. Switch back to 25.

11. **Density target.** With 25 rows on screen, all rows are visible without vertical scrolling at the app's default window size (1200×800). Measure: the table's height is under 1100 px.

12. **No regression of existing flows.** The Dashboard, PACS, Logs, Settings, and Tools tabs all still render and behave as before (Studies refactor is page-local; touching the global store added a field but did not change existing reads). Confirm by visiting each tab.

For the backend, additionally:

13. `cd src-tauri && cargo test` — all tests pass, including the new filter test added in Milestone 1.
14. `cd src-tauri && cargo clippy --all-targets -- -D warnings` — no new warnings on the changed files.

For the frontend:

15. `npm run lint` — no new errors on `app/studies/page.tsx` or `app/components/StudiesTable.tsx`.
16. `npm run build` — successful production build.

## Idempotence and Recovery

Every backend change is additive. The new `bulk_*` and `delete_all_studies` commands do not touch any existing command; the modified `current_studies` accepts an additional optional argument so old call sites continue to work unchanged (Tauri's JSON deserialisation tolerates a missing `Option<T>` field — see existing `page: Option<u32>` for precedent).

Frontend changes are confined to `app/studies/page.tsx`, the new `app/components/StudiesTable.tsx`, plus widening of `app/lib/types.ts` and `app/lib/store.ts`. Reverting the page file restores the old card view; reverting the store widening is harmless (the extra Redux field simply goes unread).

If `make dev` hangs after `delete_all_studies` because a still-in-flight send is mid-upload to Aurabox: kill the Tauri window and restart. The TUS upload resumes from its last chunk on restart (this is the existing behaviour and is not affected by this work). Note in `Surprises & Discoveries` if you observe this so the next contributor knows.

If a partially-completed implementation leaves the database in a mixed state, run the "Delete all" once the UI is restored, or directly: `sqlite3 <bounce-db-path> "DELETE FROM studies"` and `rm -rf ~/.aurabox/bounce/<studies-folder>/*`. The receiver will start populating again on the next C-STORE.

## Artifacts and Notes

Reference: the existing `current_studies` JSON shape, captured here so the frontend types match exactly. From `src-tauri/src/db/database.rs` lines 153–194:

    {
      "studies": [
        {
          "study_uid": "1.3.6.1.4.1.5962.99.1...",
          "study_description": "RM SPALLA SN",
          "study_date": "20020101",
          "study_time": "000000",
          "patient_name": "...",
          "patient_id": "...",
          "institution_name": "...",
          "accession_no": "...",
          "patient_sex": "...",
          "patient_birth_date": "...",
          "referring_physician_name": "...",
          "series_count": 1,
          "images": 12,
          "status": "SENT",
          "created_at": "2026-05-28T03:14:15Z",
          "updated_at": "2026-05-28T03:14:15Z",
          "sent_at": "2026-05-28T03:15:01Z",
          "exists": true,
          "path": "/.../studies/<uid>"
        }
      ],
      "pagination": {
        "current_page": 1,
        "total_pages": 4,
        "total_items": 87,
        "limit": 25,
        "offset": 0,
        "has_next_page": true,
        "has_previous_page": false,
        "items_on_page": 25
      }
    }

After Milestone 1 the `pagination` object additionally carries `"search": <string or null>`.

Reference: the existing Tauri command registration block at `src-tauri/src/main.rs` line 443, for orientation when adding new commands:

    .invoke_handler(tauri::generate_handler![
        receiver_start,
        receiver_stop,
        reset_app,
        send_log,
        send_study,
        delete_study,
        api_start_upload,
        current_studies,
        cfind_query,
        list_pacs_services,
        refresh_pacs_services,
        echo_pacs_service,
        show_window,
        update_send_logs,
        verify_connectivity
    ])

After Milestone 1 it must additionally contain `bulk_send_studies`, `bulk_delete_studies`, and `delete_all_studies`.

## Interfaces and Dependencies

No new Rust crates. No new npm packages — `@tauri-apps/plugin-dialog` is already aligned per commit 279bc7d ("fix(deps): align @tauri-apps/plugin-dialog with Rust crate"). Confirm by inspecting `package.json` before starting Milestone 4; if absent, install with `npm install @tauri-apps/plugin-dialog` and note in `Decision Log`.

Backend, in `src-tauri/src/db/database.rs`, the following must exist by the end of Milestone 1:

    impl Database {
        pub async fn get_studies_paginated_filtered(
            &self,
            offset: i64,
            limit: i64,
            search: Option<&str>,
        ) -> Result<(Vec<Study>, i64)>;

        pub async fn current_studies(
            &self,
            page: u32,
            limit: u32,
            search: Option<String>,
        ) -> serde_json::Value;
    }

In `src-tauri/src/main.rs`, the following Tauri commands must exist and be registered:

    #[tauri::command] async fn current_studies(app, page, limit, search) -> Result<(), String>;
    #[tauri::command] async fn bulk_send_studies(app, study_uids: Vec<String>) -> Result<(), String>;
    #[tauri::command] async fn bulk_delete_studies(app, study_uids: Vec<String>) -> Result<(), String>;
    #[tauri::command] async fn delete_all_studies(app) -> Result<(), String>;

Frontend, in `app/lib/types.ts`, the following must exist by the end of Milestone 2:

    export interface Pagination {
        current_page: number;
        total_pages: number;
        total_items: number;
        limit: number;
        offset: number;
        has_next_page: boolean;
        has_previous_page: boolean;
        items_on_page: number;
        search: string | null;
    }

    export interface Study {
        study_uid: string;
        study_description: string;
        study_date: string;
        study_time: string;
        status: string;
        exists: boolean;          // tighten from `string` — backend sends bool
        patient_name?: string;
        patient_id?: string;
        accession_no?: string;
        series_count?: number;
        images?: number;
        created_at?: string;
        updated_at?: string;
        sent_at?: string | null;
    }

    export type CurrentStudies = {
        studies: Study[];
        pagination: Pagination;
    };

In `app/lib/store.ts`, `State.pagination: Pagination | null` is added and `setCurrentStudies` writes both `studies` and `pagination`.

In `app/components/StudiesTable.tsx`, the props interface is exactly the one in Milestone 3 step 1. The component must not own any state (selection, pagination, search are all in the parent page).
