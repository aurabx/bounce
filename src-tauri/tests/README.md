# Bounce Test Suite

## Overview

This document describes the comprehensive test suite for the Bounce DICOM receiver application.

## Test Structure

Tests are organized by module and located alongside the source code:

- `src/db/database_tests.rs` - Database operations and SQLite integration tests
- `src/store/config_tests.rs` - Configuration parsing and API endpoint tests  
- `src/transmitter/transmission_tests.rs` - File compression, upload, and storage tests
- `src/receiver/metadata_tests.rs` - DICOM metadata extraction and JSON serialization tests

## Running Tests

### Run all tests
```bash
cd src-tauri
cargo test
```

### Run specific module tests
```bash
# Database tests
cargo test --test database_tests

# Config tests  
cargo test --test config_tests

# Transmission tests
cargo test --test transmission_tests

# Metadata tests
cargo test --test metadata_tests
```

### Run with output
```bash
cargo test -- --nocapture
```

### Run a specific test
```bash
cargo test test_create_study
```

## Test Coverage

### Database Tests (`database_tests.rs`)

Tests SQLite database operations:
- ✅ Study creation
- ✅ Study updates with conflict resolution
- ✅ Status updates (PENDING → SENT)
- ✅ Image count updates
- ✅ Study deletion
- ✅ Bulk clearing of studies
- ✅ Paginated queries
- ✅ Unique constraint validation

**Key Features Tested:**
- UPSERT logic with COALESCE
- Transaction handling
- DateTime field handling
- Pagination with offset/limit
- Count queries

### Config Tests (`config_tests.rs`)

Tests configuration parsing and endpoint resolution:
- ✅ API key region extraction (au, us, etc.)
- ✅ API key mode extraction (production, staging, dev, local)
- ✅ Endpoint URL generation
- ✅ Path resolution (study, archive, metadata)
- ✅ Null terminator trimming
- ✅ Invalid key handling

**Key Features Tested:**
- API key format: `aura_REGION_bounce_USER_TOKEN_MODE`
- Environment-specific endpoints
- Path sanitization

### Transmission Tests (`transmission_tests.rs`)

Tests file compression and upload functionality:
- ✅ ZIP archive creation
- ✅ Folder structure compression
- ✅ Path resolution
- ✅ Null terminator handling
- ✅ Chunk size calculations (5MB chunks)
- ✅ Upload progress calculation
- ✅ Base64 encoding for TUS metadata
- ✅ File/directory deletion
- ✅ Storage clearing
- ✅ UUID generation
- ✅ Debounce timing

**Key Features Tested:**
- WalkDir directory traversal
- ZIP compression with Deflate
- Async file operations with tokio::fs
- TUS protocol metadata encoding
- 10-second debounce logic

### Metadata Tests (`metadata_tests.rs`)

Tests DICOM metadata extraction and JSON handling:
- ✅ DCM file counting
- ✅ JSON serialization/deserialization
- ✅ Metadata file operations
- ✅ Metadata update logic
- ✅ Series tracking
- ✅ Optional field handling
- ✅ COALESCE update simulation
- ✅ Nested directory structures
- ✅ Pretty-printing JSON
- ✅ Status transitions
- ✅ Path sanitization

**Key Features Tested:**
- WalkDir with .dcm extension filtering
- Serde JSON operations
- HashMap for series tracking
- Optional DICOM fields
- Study/Series/Image hierarchy

## Test Dependencies

The test suite uses these additional dependencies (defined in `[dev-dependencies]`):

```toml
[dev-dependencies]
tempfile = "3.8"  # Temporary directories for isolated tests
```

All other test dependencies are included in the main dependencies:
- `tokio` with `full` features for async tests
- `sqlx` for database testing
- `serde_json` for JSON testing
- `walkdir` for directory traversal
- `zip` for compression testing
- `base64` for encoding tests
- `uuid` for ID generation

## Best Practices

### Test Isolation
- Each test uses `TempDir` to create isolated temporary directories
- Database tests use separate SQLite databases per test
- Tests clean up after themselves (temp dirs auto-deleted)

### Async Tests
- Use `#[tokio::test]` for async tests
- Use `tokio::fs` for async file operations
- Test both success and error cases

### Assertions
- Use descriptive assertion messages
- Test both positive and negative cases
- Validate edge cases (empty strings, null terminators, etc.)

## Common Test Patterns

### Database Test Setup
```rust
async fn setup_test_db() -> (SqlitePool, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = SqlitePool::connect(&db_url).await.expect("...");
    run_migrations(&pool).await.expect("...");
    (pool, temp_dir)
}
```

### File Test Setup
```rust
let temp_dir = TempDir::new().expect("Failed to create temp dir");
let test_file = temp_dir.path().join("test.txt");
std::fs::write(&test_file, b"content").expect("...");
```

### Async File Operations
```rust
#[tokio::test]
async fn test_async_file() {
    tokio::fs::write(&path, b"data").await.expect("...");
    let content = tokio::fs::read(&path).await.expect("...");
}
```

## Continuous Integration

These tests should be run in CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run tests
  run: cd src-tauri && cargo test --all-features
```

## Future Test Additions

Potential areas for additional testing:
- [ ] DICOM protocol integration tests (requires dcmtk)
- [ ] Network upload tests (requires mock HTTP server)
- [ ] Tauri command integration tests
- [ ] End-to-end workflow tests
- [ ] Performance/benchmark tests
- [ ] Concurrent access tests

## Troubleshooting

### Test Failures

**SQLite locked errors**: Tests may fail if SQLite database is locked. Ensure each test uses isolated databases.

**Timing issues**: Async tests may occasionally fail due to timing. Increase timeout margins if needed.

**Path issues**: Tests assume Unix-style paths. Windows may require adjustments.

### Running Tests in Development

```bash
# Watch mode - re-run tests on file changes
cargo watch -x test

# Run tests with timing info
cargo test -- --show-output --test-threads=1

# Generate coverage report (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

## Contributing

When adding new functionality:
1. Write tests alongside the implementation
2. Follow existing test patterns
3. Ensure tests are isolated and repeatable
4. Add test documentation to this README
5. Run full test suite before committing
