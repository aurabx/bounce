---
name: bounce-release
description: "Prepare and tag a release for the Bounce project. Use when the user asks to create a release, tag a release, cut a release, or bump the version. Handles the full release workflow: determining the next semver tag, bumping versions across all three config files atomically, drafting a changelog entry from commits since the last tag, updating CHANGELOG.md, committing, and creating an annotated git tag. Triggers on 'release', 'tag a release', 'cut a release', 'prepare release', 'new release', 'bump version', or any request to version or ship the current codebase."
---

# Bounce Release Workflow

## Tag Format

Tags follow the pattern `aurabox-bounce-vX.Y.Z` where `X.Y.Z` is a semantic
version number.

Examples: `aurabox-bounce-v1.2.1`, `aurabox-bounce-v1.3.0`, `aurabox-bounce-v2.0.0`

**Always include the `aurabox-bounce-v` prefix. Never use bare `vX.Y.Z` or
date-based tags.**

Semver rules:
- **Patch** (`Z`) — bug fixes, internal changes, no new user-facing features
- **Minor** (`Y`) — new features, backward-compatible
- **Major** (`X`) — breaking changes or major milestones

## Workflow

### 1. Check working tree state

```bash
git status --porcelain
```

If there are uncommitted changes **unrelated** to the release, warn the user
and ask whether to proceed. Do not silently include stray changes in the
release commit.

### 2. Determine the current version and next tag

```bash
# Current version from package.json (source of truth)
node -p "require('./package.json').version"

# Most recent Bounce release tag
git tag --list 'aurabox-bounce-v*' --sort=-version:refname | head -1
```

If the user has not specified a version, propose one based on the commits
since the last tag (patch for fixes/chores, minor for features, major for
breaking changes) and ask for confirmation before proceeding.

### 3. Collect commits since the last release tag

```bash
LAST_TAG=$(git tag --list 'aurabox-bounce-v*' --sort=-version:refname | head -1)

if [ -n "$LAST_TAG" ]; then
    git log "${LAST_TAG}..HEAD" --oneline --no-merges
else
    git log --oneline --no-merges | tail -50
fi
```

### 4. Draft the changelog entry

Group commits into sections using these headings (omit empty sections):

- **Added** — new features, new commands, new UI pages
- **Changed** — behaviour changes, refactors, dependency upgrades, config changes
- **Fixed** — bug fixes
- **Internal** — tests, CI, developer tooling, documentation, logging

Format:

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added

- Brief description of the new capability

### Fixed

- Brief description of what was broken and how it was resolved
```

Rules for drafting:
- Combine related commits into a single bullet where appropriate
- Omit pure merge commits and version-bump commits
- Keep each bullet to one concise sentence
- Use past tense ("Added", "Fixed", "Updated") — match the existing entries in
  CHANGELOG.md for style consistency
- Do not include commit hashes in the changelog

### 5. Prepend the entry to CHANGELOG.md

Insert the new entry **after** the `# Changelog` heading and the blank line
that follows it, and **before** the previous entry. Do not alter any existing
entries.

Example structure after insertion:

```
# Changelog

All notable changes to this project will be documented in this file.

## [1.3.0] - 2026-03-13

### Added

- DICOM C-MOVE support for pulling studies from remote PACS

## [1.2.1] - 2026-02-21
...
```

### 6. Bump the version atomically

Use the Makefile helper to update `package.json`, `src-tauri/Cargo.toml`, and
`src-tauri/tauri.conf.json` in one step:

```bash
make version V=X.Y.Z
```

Verify all three files reflect the new version before continuing:

```bash
node -p "require('./package.json').version"
grep '^version' src-tauri/Cargo.toml
grep '"version"' src-tauri/tauri.conf.json
```

### 7. Stage and commit the release changes

```bash
git add CHANGELOG.md package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "release: bump version to X.Y.Z"
```

Do **not** use `--no-verify`. If the pre-commit hook fails, fix the issue and
retry — do not amend.

### 8. Create an annotated tag

```bash
git tag -a "aurabox-bounce-vX.Y.Z" -m "Release aurabox-bounce-vX.Y.Z"
```

### 9. Write a short release summary

After the tag is created, output a brief human-readable summary covering:

- **Version**: the new tag
- **Release date**: today's date
- **What's new**: 2–5 bullet points drawn from the changelog entry
- **Commit**: the SHA the tag points to (`git rev-parse HEAD`)
- **Next step**: remind the user to push with `git push && git push --tags`

Example summary format:

```
Release aurabox-bounce-v1.3.0 — 2026-03-13

  - Added DICOM C-MOVE support for pulling studies from remote PACS
  - Added backend log streaming into the app UI
  - Fixed C-FIND command PDU ordering when responding to remote queries

Tagged at: abc1234
Push with: git push && git push --tags
```

## Rules

- **Never** force-push or delete an existing tag without explicit user
  instruction.
- **Never** skip hooks (`--no-verify`).
- **Never** modify `CHANGELOG.md` entries from previous releases.
- **Always** use `make version V=x.y.z` — do not edit the three version files
  individually to avoid drift.
- **Always** confirm the proposed version with the user before making any
  changes if they did not specify one.
- **Always** verify version consistency across all three files after running
  `make version`.
- If `Cargo.lock` is dirty after a version bump, stage it too — the lock file
  must stay consistent with `Cargo.toml`.
