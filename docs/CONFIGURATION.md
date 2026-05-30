# Configuration Guide

This guide provides detailed information about configuring Bounce for your environment.

## Table of Contents

- [Configuration Overview](#configuration-overview)
- [Configuration File Location](#configuration-file-location)
- [Configuration Options](#configuration-options)
- [API Key Format](#api-key-format)
- [Network Configuration](#network-configuration)
- [Storage Configuration](#storage-configuration)
- [Behavior Settings](#behavior-settings)
- [Environment-Specific Configuration](#environment-specific-configuration)
- [Configuration Examples](#configuration-examples)
- [Troubleshooting](#troubleshooting)

---

## Configuration Overview

Bounce stores its configuration using Tauri's Store plugin, which persists settings as JSON on the local filesystem. All configuration can be managed through the application's Settings page in the user interface.

**Configuration is stored at**:
- **Linux**: `~/.local/share/com.aurabox.bounce/store.json`
- **macOS**: `~/Library/Application Support/com.aurabox.bounce/store.json`
- **Windows**: `%APPDATA%\com.aurabox.bounce\store.json`

---

## Configuration File Location

The configuration file (`store.json`) is automatically created on first launch with default values. You can edit it directly or use the Settings UI.

**Example `store.json`**:
```json
{
  "api_key": "aura_au_bounce_user_token_production",
  "port": "104",
  "ip_address": "0.0.0.0",
  "ae_title": "BOUNCE",
  "base_dir": "/var/lib/bounce/storage",
  "delete_after_success": "no",
  "send_logs": "yes"
}
```

---

## Configuration Options

### `api_key` (Required)

**Type**: String  
**Default**: `""` (empty)

Your Aurabox API key for authentication. This is **required** for the application to upload studies to Aurabox.

**Format**: `aura_REGION_bounce_USER_TOKEN_ENV`

**Example**:
```json
"api_key": "aura_au_bounce_john_abc123def456_production"
```

**How to obtain**:
1. Log in to your Aurabox account
2. Navigate to Settings → API Keys
3. Generate a new "Bounce" API key
4. Copy and paste into the Settings page

**Security Notes**:
- Keep your API key confidential
- Do not commit API keys to version control
- Rotate keys periodically for security

---

### `port`

**Type**: Number (u16)  
**Default**: `9090`  
**Recommended**: `104` (standard DICOM port)

The TCP port number on which the DICOM receiver listens for incoming C-STORE requests.

**Example**:
```json
"port": "104"
```

**Notes**:
- Port 104 is the standard DICOM port but requires administrator/root privileges on most systems
- Alternative ports (e.g., 11112, 9090) can be used without elevated privileges
- Ensure the port is not already in use by another service
- Configure sending devices (PACS/modalities) to send to this port
- May require firewall rules to allow inbound connections

**Platform-specific considerations**:
- **Linux**: Ports below 1024 require root or CAP_NET_BIND_SERVICE capability
- **macOS**: Same as Linux
- **Windows**: Administrator privileges required for ports below 1024

---

### `ip_address`

**Type**: String (IPv4 address)  
**Default**: `"0.0.0.0"`

The network interface IP address to bind the DICOM receiver to.

**Example**:
```json
"ip_address": "0.0.0.0"
```

**Options**:
- `"0.0.0.0"` - Listen on all network interfaces (most common)
- `"127.0.0.1"` - Listen only on localhost (for testing)
- `"192.168.1.100"` - Listen on specific network interface

**Use cases**:
- **0.0.0.0**: Production deployments, allows connections from any network
- **127.0.0.1**: Development/testing, only local connections allowed
- **Specific IP**: Multi-homed servers, bind to specific network card

**Security consideration**: Using `0.0.0.0` exposes the DICOM receiver to all networks. Use firewall rules or specific IP binding to restrict access.

---

### `ae_title`

**Type**: String  
**Default**: `"BOUNCE"`  
**Length**: 1-16 characters (DICOM standard)

The Application Entity (AE) Title for the DICOM receiver. This identifies your Bounce instance in DICOM networks.

**Example**:
```json
"ae_title": "BOUNCE"
```

**Notes**:
- Must match the Called AE Title configured on sending devices
- Typically uppercase, but case-sensitive
- Should be unique within your DICOM network
- Cannot contain spaces (use underscores instead)
- Standard DICOM limit is 16 characters

**Common AE Titles**:
- `BOUNCE` (default)
- `BOUNCE_RAD` (radiology)
- `BOUNCE_SITE1` (multi-site deployments)
- `AURABOX_RCV`

**Configuration on sending device**:
When configuring your PACS or modality to send to Bounce, set the "Destination AE Title" or "Called AE Title" to match this value.

---

### `base_dir`

**Type**: String (filesystem path)  
**Default**: `"./tmp/dicom_storage"`

The directory path where received DICOM files are temporarily stored before upload.

**Example**:
```json
"base_dir": "/var/lib/bounce/storage"
```

**Directory structure**:
```
base_dir/
├── {StudyInstanceUID}/
│   ├── {SeriesInstanceUID}/
│   │   ├── {SOPInstanceUID}.dcm
│   │   ├── {SOPInstanceUID}.dcm
│   │   └── ...
│   └── {SeriesInstanceUID}/
│       └── ...
├── {StudyInstanceUID}.json          # Study metadata
├── {StudyInstanceUID}.zip           # Compressed study (temporary)
└── ...
```

**Recommendations**:
- Use a dedicated directory with sufficient space
- Ensure the application has read/write permissions
- Consider using a fast SSD for better performance
- Monitor disk usage if `delete_after_success` is set to "no"
- Use absolute paths for clarity

**Platform-specific suggestions**:
- **Linux**: `/var/lib/bounce/storage` or `/opt/bounce/storage`
- **macOS**: `/Users/Shared/Bounce/storage`
- **Windows**: `C:\ProgramData\Bounce\storage`

**Disk space considerations**:
- CT scans: 50-500 MB per study
- MRI scans: 100-1000 MB per study
- X-rays: 5-50 MB per study
- Ultrasound: 10-100 MB per study

Plan for adequate storage based on your expected volume and retention policy.

---

### `delete_after_success`

**Type**: String (`"yes"` or `"no"`)  
**Default**: `"no"`

Whether to automatically delete local DICOM files and archives after successful upload to Aurabox.

**Example**:
```json
"delete_after_success": "yes"
```

**Options**:

- **`"yes"`**: Delete files after successful upload
  - Saves disk space
  - Requires reliable backups in the cloud
  - No local copy for troubleshooting
  
- **`"no"`**: Keep files after upload
  - Requires manual cleanup or monitoring
  - Allows local verification and troubleshooting
  - Useful for compliance and auditing

**What gets deleted**:
1. Original DICOM files in `{StudyInstanceUID}` directory
2. Compressed ZIP archive `{StudyInstanceUID}.zip`
3. Metadata JSON file `{StudyInstanceUID}.json`

**When deletion occurs**:
- Only after upload is marked complete by Aurabox
- Never deletes if upload fails or is interrupted
- Database record is also removed

**Best practices**:
- Use `"yes"` for production with verified cloud backup
- Use `"no"` during testing or if local archiving is required
- Monitor disk space regularly if set to `"no"`

---

### `send_logs`

**Type**: String (`"yes"` or `"no"`)  
**Default**: `"no"`

Whether to send application logs to Better Stack (Logtail) for remote monitoring.

**Example**:
```json
"send_logs": "yes"
```

**Options**:

- **`"yes"`**: Send logs to remote logging service
  - Enables remote monitoring and troubleshooting
  - Helps Aurabox support team diagnose issues
  - No PHI (Protected Health Information) is sent
  - Only study UIDs and technical data are logged
  
- **`"no"`**: Keep logs local only
  - More private
  - Logs only accessible on local machine
  - Requires local access for troubleshooting

**What gets logged**:
- Application startup/shutdown events
- DICOM receiver status (started/stopped)
- Study reception events (study UID only, no patient data)
- Upload progress and completion
- Error messages and stack traces

**What is NOT logged**:
- Patient names
- Patient IDs
- DICOM file contents
- API keys or credentials
- Protected Health Information (PHI)

**Log destination**:
- **Local**: Log files in application data directory
- **Remote** (if enabled): Better Stack (Logtail) service

**Privacy considerations**:
- Remote logging does not include PHI
- Study UIDs are logged for correlation
- Hostname and IP address are included for identification
- Review your organization's privacy policy before enabling

---

### `auto_update`

**Type**: String (`"yes"` or `"no"`)
**Default**: `"no"`

Whether Bounce automatically restarts to apply an update once it has been
downloaded and installed in the background. When disabled, downloaded updates
wait for a manual restart (the existing "Restart Now" button).

When an automatic restart occurs, Bounce records whether the DICOM receiver was
running and restores that state on the next launch — a receiver that was running
is started again automatically.

**Example**:
```json
"auto_update": "yes"
```

---

### `auto_update_window_start` / `auto_update_window_end`

**Type**: String (hour `"0"`–`"23"`, local time)
**Default**: `"0"` for both

The time window during which an automatic restart is permitted. Only relevant
when `auto_update` is `"yes"`. Updates still download at any time; only the
restart waits for this window, so the service is not interrupted during busy
hours.

- If start and end are equal (e.g. both `"0"`), restarts are allowed at any
  time.
- If start is less than end (e.g. `1` to `5`), the window is `01:00`–`05:00`.
- If start is greater than end (e.g. `22` to `6`), the window wraps past
  midnight: `22:00`–`06:00`.

**Example** (restart only between 02:00 and 04:00 local time):
```json
"auto_update": "yes",
"auto_update_window_start": "2",
"auto_update_window_end": "4"
```

---

## API Key Format

The API key encodes several pieces of information in its structure:

**Format**: `aura_REGION_bounce_USER_TOKEN_ENV`

**Components**:

1. **`aura`**: Constant prefix
2. **`REGION`**: Geographic region code (e.g., `au`, `us`, `eu`)
3. **`bounce`**: Application identifier
4. **`USER`**: User or organization identifier
5. **`TOKEN`**: Random authentication token
6. **`ENV`**: Environment (optional: `production`, `staging`, `dev`, `local`)

**Examples**:

```
aura_au_bounce_johndoe_abc123_production
aura_us_bounce_clinic_xyz789_staging
aura_eu_bounce_hospital_def456_production
```

**Region Codes**:
- `au` - Australia (Sydney)
- `us` - United States
- `eu` - Europe
- `uk` - United Kingdom
- `ap` - Asia Pacific

**Environment Codes**:
- `production` (default) - Production environment
- `staging` - Staging/testing environment
- `dev` - Development environment
- `local` - Local development

**API Endpoint Resolution**:

The application automatically determines the API endpoint based on the API key:

| Environment | Endpoint |
|-------------|----------|
| production | `https://{region}.aurabox.app` |
| staging | `https://staging-5em2ouy-pghszvpk65pns.au.platformsh.site` |
| dev | `https://dev-54ta5gq-pghszvpk65pns.au.platformsh.site` |
| local | `https://aura.lndo.site` |

---

## Network Configuration

### Firewall Rules

**Inbound rules** (DICOM receiver):
```bash
# Linux (iptables)
sudo iptables -A INPUT -p tcp --dport 104 -j ACCEPT

# Linux (firewalld)
sudo firewall-cmd --permanent --add-port=104/tcp
sudo firewall-cmd --reload

# macOS
# System Preferences → Security & Privacy → Firewall → Firewall Options
# Add Bounce application

# Windows
# Windows Defender Firewall → Advanced Settings → Inbound Rules → New Rule
# Port: 104, Protocol: TCP, Action: Allow
```

**Outbound rules** (uploads to Aurabox):
- Allow HTTPS (TCP port 443) to `*.aurabox.app`
- Allow HTTPS to Better Stack (if logging enabled)

### Port Forwarding

If Bounce is behind a router/firewall:

1. Forward external port (e.g., 104) to Bounce machine IP:port
2. Configure sending devices to use external IP address
3. Ensure Bounce is listening on the correct interface (`ip_address` setting)

### Network Troubleshooting

**Test DICOM connectivity**:
```bash
# From sending machine
echoscu -v -aec BOUNCE 192.168.1.100 104

# From Bounce machine (check port is listening)
netstat -tulpn | grep 104
# or
ss -tulpn | grep 104
```

---

## Storage Configuration

### Storage Path Best Practices

1. **Use absolute paths**: Avoid relative paths like `./tmp/storage`
2. **Dedicated partition**: Consider a separate partition for DICOM storage
3. **Fast storage**: Use SSD for better performance
4. **Network storage**: Can use NFS/SMB mounts (ensure low latency)
5. **Permissions**: Ensure application has read/write access

### Disk Space Monitoring

**Calculate required space**:
```
Required Space = (Average Study Size) × (Studies per Day) × (Retention Days)
```

**Example**:
- Average CT study: 200 MB
- 50 studies per day
- 7-day retention
- Required: 200 MB × 50 × 7 = 70 GB

**Monitoring script** (Linux):
```bash
#!/bin/bash
STORAGE_DIR="/var/lib/bounce/storage"
USAGE=$(df -h "$STORAGE_DIR" | awk 'NR==2 {print $5}' | sed 's/%//')

if [ "$USAGE" -gt 80 ]; then
  echo "Warning: Storage is ${USAGE}% full"
  # Send alert
fi
```

---

## Behavior Settings

### Upload Behavior

**Debounce Period**: 10 seconds (hardcoded)

When DICOM files are received for a study, Bounce waits 10 seconds before uploading. If additional files for the same study arrive within this period, the timer resets. This ensures complete studies are uploaded together.

**Upload Process**:
1. Receive DICOM files via C-STORE
2. Save to disk and extract metadata
3. Start 10-second countdown
4. If new files arrive for same study, reset countdown
5. After 10 seconds, compress study to ZIP
6. Upload to Aurabox via TUS protocol
7. Mark upload as complete
8. Optionally delete local files

### Retry Behavior

Currently, Bounce does not automatically retry failed uploads. Failed uploads must be manually retried from the Studies page.

**Future enhancement**: Automatic retry with exponential backoff

---

## Environment-Specific Configuration

### Development Environment

```json
{
  "api_key": "aura_au_bounce_dev_token_dev",
  "port": "11112",
  "ip_address": "127.0.0.1",
  "ae_title": "BOUNCE_DEV",
  "base_dir": "./tmp/dev_storage",
  "delete_after_success": "no",
  "send_logs": "no"
}
```

**Notes**:
- Use non-privileged port (>1024)
- Bind to localhost only
- Keep files for inspection
- Disable remote logging

### Staging Environment

```json
{
  "api_key": "aura_au_bounce_org_token_staging",
  "port": "104",
  "ip_address": "0.0.0.0",
  "ae_title": "BOUNCE_STAGE",
  "base_dir": "/var/lib/bounce/storage",
  "delete_after_success": "yes",
  "send_logs": "yes"
}
```

**Notes**:
- Production-like configuration
- Use staging API endpoint
- Enable logging for testing

### Production Environment

```json
{
  "api_key": "aura_au_bounce_clinic_token_production",
  "port": "104",
  "ip_address": "0.0.0.0",
  "ae_title": "BOUNCE",
  "base_dir": "/var/lib/bounce/storage",
  "delete_after_success": "yes",
  "send_logs": "yes"
}
```

**Notes**:
- Standard DICOM port
- Listen on all interfaces
- Enable automatic cleanup
- Enable remote logging for support

---

## Configuration Examples

### Small Clinic (10-20 studies/day)

```json
{
  "api_key": "aura_us_bounce_smallclinic_abc123_production",
  "port": "104",
  "ip_address": "0.0.0.0",
  "ae_title": "BOUNCE",
  "base_dir": "/var/lib/bounce/storage",
  "delete_after_success": "yes",
  "send_logs": "yes"
}
```

**Storage**: 20 GB minimum

### Medium Hospital (50-100 studies/day)

```json
{
  "api_key": "aura_au_bounce_hospital_def456_production",
  "port": "104",
  "ip_address": "192.168.10.50",
  "ae_title": "BOUNCE_RAD",
  "base_dir": "/mnt/fast_storage/bounce",
  "delete_after_success": "no",
  "send_logs": "yes"
}
```

**Storage**: 100 GB minimum, consider dedicated partition

### Multi-Site Deployment

**Site 1**:
```json
{
  "api_key": "aura_us_bounce_org_token1_production",
  "port": "104",
  "ae_title": "BOUNCE_SITE1",
  "base_dir": "/var/lib/bounce/storage",
  "delete_after_success": "yes",
  "send_logs": "yes"
}
```

**Site 2**:
```json
{
  "api_key": "aura_us_bounce_org_token2_production",
  "port": "104",
  "ae_title": "BOUNCE_SITE2",
  "base_dir": "/var/lib/bounce/storage",
  "delete_after_success": "yes",
  "send_logs": "yes"
}
```

**Notes**: Use unique API keys or AE titles per site for identification

---

## Troubleshooting

### Configuration Not Loading

**Symptom**: Changes in Settings UI don't persist

**Solution**:
1. Check file permissions on store.json
2. Ensure application has write access to data directory
3. Check logs for errors
4. Try deleting store.json and reconfiguring

### API Key Invalid

**Symptom**: "Unauthorized" errors when uploading

**Solution**:
1. Verify API key format is correct
2. Check for extra spaces or line breaks
3. Regenerate API key in Aurabox portal
4. Ensure key is for the correct environment

### DICOM Server Won't Start

**Symptom**: "Address already in use" or "Permission denied"

**Solution**:
1. Check if port is in use: `sudo lsof -i :104`
2. Use elevated privileges for ports <1024
3. Try alternative port (e.g., 11112)
4. Check firewall settings

### Storage Directory Error

**Symptom**: "Failed to create directory" or "Permission denied"

**Solution**:
1. Ensure directory exists or can be created
2. Check filesystem permissions
3. Use absolute path instead of relative
4. Verify disk space is available

### Logs Not Appearing Remotely

**Symptom**: Local logs work but not showing in Better Stack

**Solution**:
1. Verify `send_logs` is set to "yes"
2. Check internet connectivity
3. Verify firewall allows HTTPS egress
4. API key may encode logging preferences

---

## Configuration Validation

**Check current configuration**:

From the application, you can view the effective configuration in the logs on startup:

```
[INFO] Configuration loaded:
[INFO]   API Key: aura_au_bounce_****** (redacted)
[INFO]   Port: 104
[INFO]   IP Address: 0.0.0.0
[INFO]   AE Title: BOUNCE
[INFO]   Base Directory: /var/lib/bounce/storage
[INFO]   Delete After Success: yes
[INFO]   Send Logs: yes
```

---

## Security Recommendations

1. **Protect API Keys**: Never commit to version control, use environment variables if scripting
2. **Restrict Network Access**: Use specific IP binding or firewall rules
3. **Monitor Disk Usage**: Prevent disk exhaustion attacks
4. **Regular Key Rotation**: Rotate API keys periodically
5. **Audit Logs**: Review logs regularly for suspicious activity
6. **Secure Storage**: Ensure storage directory is not world-readable
7. **Backup Configuration**: Keep secure backup of configuration

---

## Advanced Configuration

### Environment Variables (Future)

Currently, Bounce uses Tauri Store for configuration. Environment variable support may be added for containerized deployments:

```bash
export BOUNCE_API_KEY="aura_au_bounce_token"
export BOUNCE_PORT="104"
export BOUNCE_STORAGE="/data"
```

### Configuration File Backup

**Backup configuration**:
```bash
# Linux/macOS
cp ~/.local/share/com.aurabox.bounce/store.json ~/bounce-config-backup.json

# Windows
copy %APPDATA%\com.aurabox.bounce\store.json bounce-config-backup.json
```

**Restore configuration**:
```bash
# Linux/macOS
cp ~/bounce-config-backup.json ~/.local/share/com.aurabox.bounce/store.json

# Windows
copy bounce-config-backup.json %APPDATA%\com.aurabox.bounce\store.json
```

---

## Support

For configuration assistance:
- **Documentation**: https://docs.aurabox.cloud/applications/bounce/
- **Email**: support@aurabox.cloud
- **GitHub Issues**: https://github.com/aurabx/bounce/issues
