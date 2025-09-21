#!/bin/bash

# Paths to the files that need version updates
PACKAGE_JSON="package.json"
CARGO_TOML="src-tauri/Cargo.toml"
TAURI_CONF="src-tauri/tauri.conf.json"

# Function to update the version number in the specified files
update_version() {
  if [[ -z "$1" ]]; then
    echo "Error: Version number not provided."
    echo "Usage: $0 <new_version>"
    exit 1
  fi

  NEW_VERSION=$1
  echo "Updating version number to $NEW_VERSION..."

  # Update version in package.json
  sed -i.bak "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" $PACKAGE_JSON && rm -f ${PACKAGE_JSON}.bak
  echo "Updated version in $PACKAGE_JSON."

  # Update version in Cargo.toml
  sed -i.bak "s/^version = \".*\"/version = \"$NEW_VERSION\"/" $CARGO_TOML && rm -f ${CARGO_TOML}.bak
  echo "Updated version in $CARGO_TOML."

  # Update version in tauri.conf.json
  sed -i.bak "s/\"version\": \".*\"/\"version\": \"$NEW_VERSION\"/" $TAURI_CONF && rm -f ${TAURI_CONF}.bak
  echo "Updated version in $TAURI_CONF."

  echo "Version update completed."
}

# Main script execution
if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <new_version>"
  exit 1
fi

update_version $1