# Bounce Testing Quick Start

## Run All Tests
```bash
cd src-tauri
cargo test
```

## Test Results
✅ **58 tests** (51 unit + 7 integration)  
⏱️ **~0.11 seconds**

## Test Files

```
src/
├── db/
│   └── database_tests.rs        # 10 tests - Database operations
├── store/
│   └── config_tests.rs          # 17 tests - Config parsing
├── transmitter/
│   └── transmission_tests.rs    # 14 tests - File ops & uploads
└── receiver/
    └── metadata_tests.rs        # 10 tests - DICOM metadata

tests/
└── integration_test.rs          #  7 tests - Workflows
```

## Common Commands

```bash
# Run specific module
cargo test db::database_tests
cargo test store::config_tests
cargo test transmitter::transmission_tests
cargo test receiver::metadata_tests

# Run integration tests only
cargo test --test integration_test

# Run with output
cargo test -- --nocapture

# Run single test
cargo test test_create_study

# Watch mode (requires cargo-watch)
cargo watch -x test
```

## What's Tested

### Database (10 tests)
- ✅ CRUD operations
- ✅ Pagination
- ✅ UPSERT with COALESCE
- ✅ Unique constraints

### Config (17 tests)
- ✅ API key parsing
- ✅ Endpoint resolution
- ✅ Path handling
- ✅ Edge cases

### Transmission (14 tests)
- ✅ ZIP compression
- ✅ Upload chunking (5MB)
- ✅ TUS protocol
- ✅ File operations

### Metadata (10 tests)
- ✅ JSON operations
- ✅ DICOM fields
- ✅ Series tracking
- ✅ File counting

### Integration (7 tests)
- ✅ Complete workflows
- ✅ Concurrent operations
- ✅ Path resolution
- ✅ Status lifecycle

## Dependencies

Already installed in main dependencies:
- `tokio`, `sqlx`, `serde_json`, `walkdir`, `zip`, `base64`, `uuid`, `chrono`

Dev dependencies (added):
```toml
[dev-dependencies]
tempfile = "3.8"
futures = "0.3"
```

## Documentation

- **Full details:** `tests/README.md`
- **Summary:** `TEST_SUMMARY.md` (project root)
- **This file:** Quick reference

## Adding New Tests

1. Create `module_tests.rs` in module directory
2. Add to `mod.rs`: `#[cfg(test)] mod module_tests;`
3. Follow existing patterns (see test files)
4. Run `cargo test` to verify

## Test Patterns

### Database Test
```rust
#[tokio::test]
async fn test_something() {
    let (pool, _temp) = setup_test_db().await;
    // Test code
}
```

### File Test
```rust
#[test]
fn test_something() {
    let temp_dir = TempDir::new().expect("...");
    let path = temp_dir.path().join("file.txt");
    // Test code
}
```

### Async Test
```rust
#[tokio::test]
async fn test_something() {
    tokio::fs::write(&path, b"data").await.expect("...");
    // Test code
}
```

## CI Integration

```yaml
- name: Run tests
  run: cd src-tauri && cargo test --all-features
```

---

**Last Updated:** Test suite created with 58 passing tests  
**Status:** ✅ All tests passing
