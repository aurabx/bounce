# Bounce

**Bounce** is a lightweight DICOM C-STORE receiver that forwards received files to Aurabox. It is designed to run behind a healthcare provider's firewall and securely forward medical imaging to [Aurabox](https://aurabox.cloud).

## Features

- Accepts inbound DICOM C-STORE requests
- Forwards received DICOM files to the Aurabox backend over HTTPS
- Logs DICOM metadata as part of the forwarding process
- Designed for secure, internal deployments
- Minimal configuration required

---

## 🚀 Getting Started (Production Use)

### 1. Download the Latest Release

Visit the [Releases page](https://github.com/aurabx/bounce/releases) and download the latest `.tar.gz` or binary appropriate for your platform.

### 2. Set up

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
```shell
npm install
npx tauri dev
```

### Build the UI

1. Build the UI
```
$ npm run build
```

2. Run Tauri locally

```
$ next dev
```

3. Or, build the app

```
npm install
npm run tauri dev
```
