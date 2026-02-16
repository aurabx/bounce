.PHONY: dev dev-frontend dev-backend build build-frontend build-release \
       test test-rust lint lint-frontend lint-rust fmt fmt-rust \
       clean clean-frontend clean-rust clean-all \
       install check version dicom-echo dicom-send help

# Default target
.DEFAULT_GOAL := help

# -------------------------------------------------------------------
# Development
# -------------------------------------------------------------------

## Start the full Tauri application with hot-reload (frontend + backend)
dev:
	npm run tauri:dev

## Start the Next.js frontend dev server only
dev-frontend:
	npm run dev

## Run the Rust backend only (without Tauri window manager)
dev-backend:
	cargo run --manifest-path=src-tauri/Cargo.toml

# -------------------------------------------------------------------
# Building
# -------------------------------------------------------------------

## Build the Next.js frontend to static files
build-frontend:
	npm run build

## Build the complete Tauri application for release
build-release:
	npm run tauri:build

## Alias: build frontend then full Tauri app
build: build-frontend build-release

# -------------------------------------------------------------------
# Testing
# -------------------------------------------------------------------

## Run Rust unit tests
test-rust:
	cd src-tauri && cargo test

## Run all tests (currently Rust only)
test: test-rust

# -------------------------------------------------------------------
# Linting & Formatting
# -------------------------------------------------------------------

## Lint the Next.js frontend (ESLint)
lint-frontend:
	npm run lint

## Lint the Rust backend (Clippy)
lint-rust:
	cd src-tauri && cargo clippy -- -D warnings

## Run all linters
lint: lint-frontend lint-rust

## Format Rust code
fmt-rust:
	cd src-tauri && cargo fmt

## Check Rust formatting without modifying files
fmt-check:
	cd src-tauri && cargo fmt -- --check

## Format all code
fmt: fmt-rust

# -------------------------------------------------------------------
# Cleaning
# -------------------------------------------------------------------

## Remove Next.js build output and cache
clean-frontend:
	rm -rf .next out

## Remove Rust build artifacts
clean-rust:
	cd src-tauri && cargo clean

## Remove all build artifacts (frontend + backend + node_modules)
clean-all: clean-frontend clean-rust
	rm -rf node_modules

## Alias for cleaning frontend and backend (keeps node_modules)
clean: clean-frontend clean-rust

# -------------------------------------------------------------------
# Setup & Utilities
# -------------------------------------------------------------------

## Install all dependencies (Node + Rust)
install:
	npm install
	cd src-tauri && cargo fetch

## Run pre-commit checks (lint + format check + tests)
check: lint fmt-check test

## Update version across package.json, Cargo.toml, and tauri.conf.json
## Usage: make version V=1.2.3
version:
ifndef V
	$(error Usage: make version V=1.2.3)
endif
	./update-version.sh $(V)

# -------------------------------------------------------------------
# DICOM Testing
# -------------------------------------------------------------------

## Test DICOM connectivity with C-ECHO (requires DCMTK)
## Usage: make dicom-echo [PORT=104] [AE=BOUNCE]
dicom-echo:
	echoscu -v -aec $(or $(AE),BOUNCE) localhost $(or $(PORT),104)

## Send a DICOM file via C-STORE (requires DCMTK)
## Usage: make dicom-send FILE=/path/to/file.dcm [PORT=104] [AE=BOUNCE]
dicom-send:
ifndef FILE
	$(error Usage: make dicom-send FILE=/path/to/file.dcm [PORT=104] [AE=BOUNCE])
endif
	storescu -v -aec $(or $(AE),BOUNCE) localhost $(or $(PORT),104) $(FILE)

# -------------------------------------------------------------------
# Help
# -------------------------------------------------------------------

## Show this help message
help:
	@echo "Bounce - DICOM C-STORE Receiver"
	@echo ""
	@echo "Usage: make <target>"
	@echo ""
	@echo "Development:"
	@echo "  dev              Start full Tauri app with hot-reload"
	@echo "  dev-frontend     Start Next.js frontend dev server only"
	@echo "  dev-backend      Run Rust backend only"
	@echo ""
	@echo "Building:"
	@echo "  build            Build frontend and full Tauri app"
	@echo "  build-frontend   Build Next.js frontend only"
	@echo "  build-release    Build complete Tauri app for release"
	@echo ""
	@echo "Testing:"
	@echo "  test             Run all tests"
	@echo "  test-rust        Run Rust unit tests"
	@echo ""
	@echo "Linting & Formatting:"
	@echo "  lint             Run all linters"
	@echo "  lint-frontend    Lint frontend (ESLint)"
	@echo "  lint-rust        Lint backend (Clippy)"
	@echo "  fmt              Format all code"
	@echo "  fmt-rust         Format Rust code"
	@echo "  fmt-check        Check Rust formatting"
	@echo ""
	@echo "Cleaning:"
	@echo "  clean            Remove build artifacts"
	@echo "  clean-frontend   Remove Next.js cache and output"
	@echo "  clean-rust       Remove Rust build artifacts"
	@echo "  clean-all        Remove everything (including node_modules)"
	@echo ""
	@echo "Utilities:"
	@echo "  install          Install all dependencies"
	@echo "  check            Run lint + format check + tests"
	@echo "  version V=x.y.z  Update version across all configs"
	@echo ""
	@echo "DICOM Testing (requires DCMTK):"
	@echo "  dicom-echo               Test connectivity"
	@echo "  dicom-send FILE=path.dcm Send a DICOM file"
