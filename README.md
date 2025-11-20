# Bounce

<div align="center">

**A lightweight DICOM C-STORE receiver that securely forwards medical imaging to Aurabox**

[![License: BOUNCE EULA](https://img.shields.io/badge/License-Proprietary-red.svg)](LICENSE)
[![Platform: Windows | macOS | Linux](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-blue.svg)]()

</div>

---

## 📖 Overview

**Bounce** is a cross-platform desktop application designed to bridge the gap between on-premises medical imaging equipment and cloud-based DICOM storage. Built with Tauri and Rust, Bounce runs behind healthcare providers' firewalls to receive DICOM files via the C-STORE protocol and securely forward them to [Aurabox](https://aurabox.cloud) over HTTPS.

### Key Capabilities

- **DICOM C-STORE Receiver**: Accepts inbound DICOM C-STORE requests from PACS, modalities, and other DICOM sources
- **Secure Cloud Upload**: Forwards received DICOM files to Aurabox backend using TUS protocol over HTTPS
- **Metadata Extraction**: Automatically extracts and logs DICOM metadata (Study UID, Series UID, Patient info, etc.)
- **Local Storage Management**: Temporarily stores DICOM files locally with configurable retention policies
- **Study Aggregation**: Intelligently groups DICOM instances into studies with debouncing logic
- **Compression**: Automatically compresses studies into ZIP archives before upload
- **Desktop UI**: Modern web-based interface for monitoring, configuration, and study management
- **System Tray Integration**: Runs in the background with system tray icon for quick access
- **SQLite Database**: Tracks study status and transmission history
- **Logging**: Comprehensive logging with optional remote logging to Better Stack

---

## 🚀 Quick Start

### Installation

1. Visit the [Releases page](https://github.com/aurabx/bounce/releases)
2. Download the latest installer for your platform:
   - **Windows**: `.msi` or `.exe` installer
   - **macOS**: `.dmg` disk image
   - **Linux**: `.deb`, `.AppImage`, or `.tar.gz`
3. Run the installer and follow the setup wizard

### Configuration

1. Launch Bounce application
2. Navigate to Settings
3. Configure the following:
   - **API Key**: Your Aurabox API key (required)
   - **AE Title**: Application Entity title for DICOM (default: `BOUNCE`)
   - **Port**: DICOM receiver port (default: `104`)
   - **IP Address**: Network interface to bind to (default: `0.0.0.0`)
   - **Storage Location**: Directory for temporary DICOM file storage
   - **Delete After Send**: Automatically delete files after successful upload

4. Click "Start Server" to begin receiving DICOM files

For detailed setup instructions, visit: https://docs.aurabox.cloud/applications/bounce/

---

## 🧪 Testing

### Send Test DICOM Files

Use `storescu` from [DCMTK](https://dicom.offis.de/dcmtk.php.en) to send test DICOM files:

```bash
# Send a single DICOM file
storescu -aec BOUNCE 127.0.0.1 104 /path/to/test.dcm

# Send multiple files
storescu -aec BOUNCE 127.0.0.1 104 /path/to/dicom/folder/*.dcm

# Send with verbose output
storescu -v -aec BOUNCE 127.0.0.1 104 /path/to/test.dcm
```

### Verify C-ECHO (Connection Test)

```bash
echoscu -aec BOUNCE 127.0.0.1 104
```

---

## 🛠 Development

### Prerequisites

- **Node.js** 18+ and npm
- **Rust** (latest stable) and Cargo
- **Tauri CLI** (installed via npm)
- **Platform-specific dependencies**:
  - **Linux**: `libssl-dev`, `libsqlite3-dev`, `webkit2gtk-4.1-dev`
  - **macOS**: Xcode Command Line Tools
  - **Windows**: Visual Studio Build Tools

### Setup Development Environment

```bash
# Clone the repository
git clone https://github.com/aurabx/bounce.git
cd bounce

# Install Node dependencies
npm install

# Run in development mode
npm run tauri:dev
```

The application will launch with hot-reload enabled for both the frontend and backend.

### Project Structure

```
bounce/
├── app/                    # Next.js frontend application
│   ├── components/         # React components
│   ├── lib/               # Frontend utilities and helpers
│   ├── logs/              # Logs page
│   ├── settings/          # Settings page
│   ├── studies/           # Studies management page
│   └── tools/             # Tools page
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── aura/          # Aurabox API client
│   │   ├── db/            # SQLite database layer
│   │   ├── receiver/      # DICOM C-STORE receiver
│   │   ├── transmitter/   # Upload/transmission logic
│   │   ├── store/         # Configuration management
│   │   ├── lib/           # Utility modules
│   │   └── main.rs        # Application entry point
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
├── package.json           # Node.js dependencies and scripts
└── README.md
```

### Available Commands

```bash
# Development
npm run dev              # Run Next.js dev server only
npm run tauri:dev        # Run full Tauri app in dev mode

# Building
npm run build            # Build Next.js frontend
npm run tauri:build      # Build Tauri application for release

# Linting
npm run lint             # Run ESLint

# Version Management
./update-version.sh 1.2.3  # Update version across all config files
```

### Building for Release

```bash
# Update version number
./update-version.sh 1.2.3

# Build release binaries
npm run tauri:build
```

Built applications will be in `src-tauri/target/release/bundle/`

---

## 📚 Documentation

- **[Architecture Overview](./docs/ARCHITECTURE.md)** - System design and component interaction
- **[Development Guide](./docs/DEVELOPMENT.md)** - Detailed development setup and guidelines  
- **[Configuration Guide](./docs/CONFIGURATION.md)** - Configuration options and settings
- **[API Reference](./docs/API.md)** - Tauri commands and API documentation
- **[User Documentation](https://docs.aurabox.cloud/applications/bounce/)** - End-user guide

---

## 🔐 Security

Bounce is designed for secure deployments in healthcare environments:

- All uploads to Aurabox use HTTPS with TLS 1.2+
- API key authentication for all cloud communications
- Local storage uses filesystem permissions for access control
- No PHI (Protected Health Information) is logged in plain text
- Optional automatic deletion of files after successful upload

---

## 🐛 Troubleshooting

### DICOM Server Won't Start

- Check if port 104 is available (may require admin/sudo privileges)
- Verify firewall rules allow inbound connections on configured port
- Check logs in the application's Logs tab

### Files Not Uploading

- Verify API key is correctly configured
- Check internet connectivity to Aurabox
- Review upload status in Studies tab
- Check logs for error messages

### Application Won't Launch

- Ensure all dependencies are installed
- Check system compatibility (Windows 10+, macOS 10.13+, recent Linux)
- Try running from terminal to see error messages

---

## 📝 License

See [BOUNCE EULA](LICENSE).

---

## 🙋 Support

For issues, questions, or feature requests:

- **Email**: support@aurabox.cloud
- **Documentation**: https://docs.aurabox.cloud
- **GitHub Issues**: https://github.com/aurabx/bounce/issues (for bug reports)

---

## 🙏 Acknowledgments

Built with:
- [Tauri](https://tauri.app/) - Desktop application framework
- [Next.js](https://nextjs.org/) - React frontend framework
- [Rust DICOM](https://github.com/Enet4/dicom-rs) - DICOM protocol implementation
- [TUS Protocol](https://tus.io/) - Resumable file upload protocol
