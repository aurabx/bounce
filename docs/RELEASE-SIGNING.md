# Release Signing

This guide documents the codesigning, notarization, and Tauri updater
signing setup used by Bounce's release pipeline. It covers what each
secret is, where the underlying material lives, how to produce the
secrets from scratch, and how to rotate them.

Releases are built and published by
[`.github/workflows/publish.yml`](../.github/workflows/publish.yml).
That workflow consumes the GitHub repository secrets described below.

## Table of Contents

- [Required Secrets](#required-secrets)
- [Apple Developer ID (codesigning)](#apple-developer-id-codesigning)
- [Apple Notarization Credentials](#apple-notarization-credentials)
- [Tauri Updater Signing Key](#tauri-updater-signing-key)
- [Generating a New Tauri Key for Another App](#generating-a-new-tauri-key-for-another-app)
- [Known Issues](#known-issues)
- [Renewal Calendar](#renewal-calendar)

---

## Required Secrets

The release workflow expects the following secrets on the GitHub
repository. Missing or invalid values will cause the macOS jobs in
particular to fail.

| Secret | Used for | Notes |
|---|---|---|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` containing the Aurabox Developer ID Application cert and its private key. | macOS codesigning. See below. |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12`. | macOS codesigning. |
| `KEYCHAIN_PASSWORD` | Throwaway password used to lock the temporary build keychain created during the workflow. | Arbitrary value; not reused outside the workflow run. |
| `APPLE_ID` | Apple ID email enrolled in the Aurabox developer team. | Notarization. |
| `APPLE_ID_PASSWORD` | App-specific password for the Apple ID above. | Notarization. |
| `APPLE_PASSWORD` | Set to the same value as `APPLE_ID_PASSWORD`. The Tauri action reads `APPLE_PASSWORD`; the workflow passes both names to remain compatible across action versions. | Notarization. |
| `APPLE_TEAM_ID` | `X52XM4SP3U` (Aurabox Pty Ltd). | Notarization. |
| `TAURI_SIGNING_PRIVATE_KEY` | Contents of the rsign2 / minisign-style encrypted private key used to sign updater artifacts. | Updater. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Passphrase used when the key was generated. | Updater. |

The Apple signing identity is hardcoded in
[`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) as
`Developer ID Application: Aurabox Pty Ltd (X52XM4SP3U)`. The Tauri
updater **public** key is also committed there under
`plugins.updater.pubkey` — it is non-sensitive (it only verifies
signatures, never produces them) and must match the private key in
`TAURI_SIGNING_PRIVATE_KEY`.

---

## Apple Developer ID (codesigning)

Bounce's macOS builds are signed with the Aurabox organisational
Developer ID Application certificate:

- **Identity:** `Developer ID Application: Aurabox Pty Ltd (X52XM4SP3U)`
- **SHA-1 fingerprint:** `1ECEB9B5CEAEEF29810416CB3D9C629582045D20`
- **Issuer:** Apple `Developer ID Certification Authority`
- **Validity:** issued 2022-06-20, expires **2027-02-01**

The certificate and its private key live in the macOS login keychain
of whoever holds the Aurabox Developer ID enrolment. Confirm presence
with:

```bash
security find-identity -v -p codesigning
```

The first column of the matching row is the SHA-1 fingerprint above.

### Producing the `APPLE_CERTIFICATE` secret

The workflow expects a base64-encoded `.p12` containing the cert and
the matching private key. The cleanest way to produce this is to
export the identity from the keychain that already holds it.

**Option A: Keychain Access GUI.** Open Keychain Access, locate the
`Developer ID Application: Aurabox Pty Ltd` entry under *My
Certificates*, right-click, choose *Export*, and save as `.p12` with
a new password. Then:

```bash
base64 -i path/to/exported.p12 -o exported.p12.b64
```

**Option B: CLI export with explicit single-identity output.** This
matches the `security` import format the workflow uses on the CI
runner (legacy PKCS12 / RC2-40-CBC; required by macOS's `security
import`). It assumes the Aurabox identity is in the login keychain.

```bash
# 1. Bulk-export all identities from the login keychain.
#    macOS will show GUI prompts asking you to allow access to each
#    private key; click Always Allow.
P12_PASSWORD=$(openssl rand -base64 18)
echo "$P12_PASSWORD" > p12-password.txt
chmod 600 p12-password.txt

security export \
  -k ~/Library/Keychains/login.keychain-db \
  -t identities \
  -f pkcs12 \
  -P "$P12_PASSWORD" \
  -o all-identities.p12

# 2. Slice out the Aurabox cert and matching private key.
openssl pkcs12 -legacy -in all-identities.p12 -out all.pem -nodes \
  -passin "pass:$P12_PASSWORD"

python3 <<'PYEOF'
import re, pathlib
text = pathlib.Path("all.pem").read_text()
pattern = re.compile(
    r"(?:Bag Attributes.*?friendlyName:\s*(?P<name>[^\n]+).*?)?"
    r"(?P<pem>-----BEGIN (?P<kind>[A-Z ]+)-----.*?-----END \3-----)",
    re.DOTALL,
)
cert = key = None
for m in pattern.finditer(text):
    name = (m.group("name") or "").strip()
    kind = m.group("kind").strip()
    if "Aurabox" in name and "CERTIFICATE" in kind:
        cert = m.group("pem")
    elif "Aurabox" in name and "PRIVATE KEY" in kind:
        key = m.group("pem")
assert cert and key
pathlib.Path("aurabox_cert.pem").write_text(cert + "\n")
pathlib.Path("aurabox_key.pem").write_text(key + "\n")
PYEOF

# 3. Repack as a clean single-identity .p12 in legacy format.
#    The -legacy flag is required: macOS's `security import` rejects
#    .p12 files built with OpenSSL 3's default modern ciphers.
openssl pkcs12 -export -legacy \
  -in aurabox_cert.pem \
  -inkey aurabox_key.pem \
  -name "Developer ID Application: Aurabox Pty Ltd (X52XM4SP3U)" \
  -passout "pass:$P12_PASSWORD" \
  -out aurabox-developer-id.p12

# 4. Base64-encode for the GitHub secret.
base64 -i aurabox-developer-id.p12 -o aurabox-developer-id.p12.b64

# 5. Shred the unencrypted intermediates.
rm -P all.pem aurabox_key.pem
rm -f all-identities.p12 aurabox_cert.pem
```

Set the secrets:

- `APPLE_CERTIFICATE` = contents of `aurabox-developer-id.p12.b64`
- `APPLE_CERTIFICATE_PASSWORD` = contents of `p12-password.txt`

### Verifying the `.p12` before pushing it as a secret

Simulate exactly what the CI runner does:

```bash
TMPKEY="/tmp/verify-$$.keychain-db"
KCPW="throwaway"
security create-keychain -p "$KCPW" "$TMPKEY"
ORIG=$(security list-keychains -d user | xargs)
security list-keychains -d user -s "$TMPKEY" $ORIG
security unlock-keychain -p "$KCPW" "$TMPKEY"
security import aurabox-developer-id.p12 -k "$TMPKEY" \
  -P "$P12_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list \
  -S apple-tool:,apple:,codesign: -s -k "$KCPW" "$TMPKEY"
security find-identity -v -p codesigning "$TMPKEY"
# Expect exactly one identity with the SHA-1 above.
security list-keychains -d user -s $ORIG
security delete-keychain "$TMPKEY"
```

If `find-identity` reports `0 valid identities found`, the `.p12` is
in the wrong format (most often: built without `-legacy`).

---

## Apple Notarization Credentials

After codesigning, the workflow submits the bundles to Apple for
notarization via `tauri-action`, which in turn invokes `notarytool`.

### App-specific password

`APPLE_ID_PASSWORD` (and the mirror `APPLE_PASSWORD`) must be an
**app-specific password**, not the regular Apple ID account password.
Generate one at <https://appleid.apple.com> → *Sign-In and Security*
→ *App-Specific Passwords*. Label it something traceable like
`bounce-ci-notarization`.

The Apple ID used must be a member of the Aurabox developer team
(`APPLE_TEAM_ID=X52XM4SP3U`) with permission to submit notarization
requests.

### Rotating the notarization password

App-specific passwords can be revoked individually at appleid.apple.com
without affecting the account or other passwords. Rotation steps:

1. Generate a new app-specific password.
2. Update `APPLE_ID_PASSWORD` and `APPLE_PASSWORD` to the new value.
3. Revoke the old password.
4. Trigger a release build to confirm notarization still succeeds.

---

## Tauri Updater Signing Key

The Tauri updater verifies update artifacts (`.app.tar.gz`,
`.AppImage`, NSIS installer) with a minisign-compatible signature.
The private key is held by the release maintainer and used by CI; the
matching public key is embedded in every shipped binary and used by
running installs to decide whether to trust an update.

The Bounce key lives locally at:

- `~/.tauri/bounce.key` — encrypted private key
- `~/.tauri/bounce.key.pub` — public key

The contents of `bounce.key` (the whole base64 blob, including its
header line) are stored in CI as `TAURI_SIGNING_PRIVATE_KEY`, and the
passphrase used at key creation is stored as
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

### Why the key matters

- **Lose the key file:** you can no longer publish updates that
  existing installs will accept. Users must reinstall a build with a
  new public key baked in.
- **Lose only the passphrase:** the encrypted key file is useless;
  same outcome as losing the file.
- **Leak the key + passphrase:** an attacker can sign and publish
  updates that every Bounce install will accept as legitimate.

Store both the private key file and the passphrase in a secrets
manager. They should not live only on a single developer's laptop.

---

## Generating a New Tauri Key for Another App

The same key must never be reused across apps; the public key baked
into each binary is what scopes update trust.

```bash
npx tauri signer generate -w ~/.tauri/<app-name>.key
# Interactive: prompts for a passphrase.

# Or non-interactive:
npx tauri signer generate \
  -w ~/.tauri/<app-name>.key \
  -p '<strong-passphrase>' \
  --ci
```

This produces two files:

| File | Sensitivity | Purpose |
|---|---|---|
| `~/.tauri/<app-name>.key` | Secret | Encrypted private key. Goes into `TAURI_SIGNING_PRIVATE_KEY`. |
| `~/.tauri/<app-name>.key.pub` | Public | Public key. Goes into the app's `tauri.conf.json`. |

Wire into the new app:

1. Paste the entire `.pub` file body into the new app's
   `src-tauri/tauri.conf.json` under
   `plugins.updater.pubkey` — Tauri reads it from there at build
   time to bake into the binary.
2. Set the CI secrets:
   - `TAURI_SIGNING_PRIVATE_KEY` = full contents of
     `~/.tauri/<app-name>.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the passphrase
3. Optional smoke test:
   ```bash
   echo "test" > /tmp/payload.txt
   npx tauri signer sign \
     -k ~/.tauri/<app-name>.key \
     -p '<passphrase>' \
     /tmp/payload.txt
   # Produces /tmp/payload.txt.sig
   ```

---

## Known Issues

### 1. `APPLE_ID_PASSWORD` and `APPLE_PASSWORD` duplicate each other

The workflow exposes both env vars with what is expected to be the
same value. This is defensive: different versions of
`tauri-apps/tauri-action` and Apple's `notarytool` invocation paths
have historically read one or the other. Keep both secrets in sync
when rotating the app-specific password.

---

## Renewal Calendar

| Item | Expires / Renewal trigger |
|---|---|
| Aurabox Developer ID Application certificate | **2027-02-01** |
| Apple ID app-specific password | No expiry, but rotate annually as a hygiene measure |
| Tauri updater key | No expiry; rotation requires shipping a new binary with the new pubkey and is therefore a coordinated release |

Set a calendar reminder for **November 2026** to renew the Developer
ID certificate before it expires. Renewal flow:

1. Sign in to <https://developer.apple.com> with an account in the
   Aurabox team.
2. Under *Certificates, Identifiers & Profiles*, request a new
   *Developer ID Application* certificate. Apple will ask for a CSR.
3. Generate the CSR from Keychain Access (*Keychain Access* → menu
   *Certificate Assistant* → *Request a Certificate From a Certificate
   Authority*; save to disk). The private key is created and stored
   in the login keychain at this point.
4. Upload the CSR; download the issued `.cer`; double-click to install
   in the login keychain. It will join the existing private key to
   form a usable signing identity.
5. Export a new `.p12` and update the `APPLE_CERTIFICATE` /
   `APPLE_CERTIFICATE_PASSWORD` secrets following the steps in
   [Producing the `APPLE_CERTIFICATE` secret](#producing-the-apple_certificate-secret).
6. Trigger a release build and confirm the produced bundle is signed
   by the new certificate (`codesign -dv --verbose=4
   /path/to/Bounce.app` should show the new validity dates).
