# Bounce

**Bounce** is a lightweight DICOM C-STORE receiver that forwards received files to Aurabox. It is designed to run behind a healthcare provider's firewall and securely forward medical imaging to [Aurabox](https://aurabox.cloud).

## Features

- Accepts inbound DICOM C-STORE requests
- Forwards received DICOM files to the Aurabox backend over HTTPS
- Logs DICOM metadata as part of the forwarding process
- Designed for secure, internal deployments
- Minimal configuration required

## Installation

Visit the [Releases page](https://github.com/aurabx/bounce/releases) and download the latest `.tar.gz` or binary appropriate for your platform.

## Set up

Follow the instructions at https://docs.aurabox.cloud/applications/bounce/ to complete the install.


Here is the raw Markdown version of the `README.md`:

## 🧪 Testing

To simulate a C-STORE transfer, use a tool like `storescu` from DCMTK:

```bash
storescu -aec BOUNCE 127.0.0.1 104 /path/to/test.dcm
```

---

## 🛠 Developer Information

### Prerequisites

* Rust (latest stable)
* Cargo

### Build

1. Install and run
```bash
npm install
npx tauri dev
```

### Build the UI

1. Build the UI
```bash
npm run build
```

2. Run Tauri locally

```bash
next dev
```

3. Or, build the app

```bash
npm install
npm run tauri dev
```

### Build for release

1. Update the version number in package.json, Cargo.toml and tauri.conf.json, e.g.

```bash
./update-version.sh 1.0.0
```
