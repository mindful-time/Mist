# Apple signing setup for Mist

Checked against Apple's documentation on 2026-10-02. Never put signing files,
encoded certificates, or passwords in chat, issues, commits, or logs.

## 1. Check the account and certificate type

A free developer account is insufficient: Developer ID and notarization need
Apple Developer Program membership. Check your active membership in your
[developer account](https://developer.apple.com/account/).
([Apple membership comparison](https://developer.apple.com/support/compare-memberships/))

Mist's app/DMG needs **Developer ID Application**, not Apple Development, Apple
Distribution, or Developer ID Installer (`.pkg` only). Local Developer ID
creation requires the Account Holder; individual members hold that role.
([Apple Developer ID instructions](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/),
[account roles](https://developer.apple.com/help/account/access/roles/))

## 2. Create a local signing identity, if needed

Already have a valid Developer ID Application identity **with its private key
on this Mac**? Reuse it and skip to step 3.

In Xcode:

1. Open **Xcode → Settings → Accounts** and add/sign in to your Apple Account.
2. Select the enrolled team, then **Manage Certificates**.
3. Click **+ → Developer ID Application**, then **Done**.

The matching private key stays in your login keychain; no Xcode project is needed.
([Apple's current Xcode instructions](https://developer.apple.com/documentation/xcode/sharing-your-teams-signing-certificates),
[Developer ID option](https://help.apple.com/xcode/mac/current/en.lproj/dev88332a81e.html))

Portal alternative: use **Keychain Access → Certificate Assistant → Request a
Certificate from a Certificate Authority**. Enter email/key name, leave CA email
empty, choose **Saved to disk**. In Apple's **Certificates, Identifiers & Profiles
→ Certificates → +**, select **Developer ID / Developer ID Application**, upload
the CSR, and download the `.cer`. Double-click it on the **CSR-generating Mac**.
([Apple CSR steps](https://developer.apple.com/help/account/certificates/create-a-certificate-signing-request),
[portal steps](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/))

## 3. Export `DeveloperIDApplication.p12`

1. Open **Keychain Access → login → My Certificates**.
2. Expand the correct **Developer ID Application** entry: its **private key**
   must appear beneath it.
3. Select the certificate and associated key; choose **File → Export Items**.
4. Choose **Personal Information Exchange (.p12)**; save
   `DeveloperIDApplication.p12` securely **outside the repository**.
5. Set/store a strong export password and approve any macOS authentication
   prompt. This becomes `APPLE_CERTIFICATE_PASSWORD`, not your Apple/Mac password.

Alternatively, Control-click the identity in Xcode's Manage Certificates sheet
and choose **Export Certificate** for a password-protected PKCS#12 export.
([Apple Keychain export steps](https://support.apple.com/guide/keychain-access/import-and-export-keychain-items-kyca35961/mac),
[certificate-plus-key export pattern](https://developer.apple.com/help/account/capabilities/communicate-with-apns-using-a-tls-certificate/),
[Xcode export](https://developer.apple.com/documentation/xcode/sharing-your-teams-signing-certificates))

No private key or `.p12` option? A `.cer` alone cannot restore it; renaming it
doesn't help. Recover from the original Mac/secure backup, or ask the Account
Holder to create a new local certificate. Do **not revoke** existing certificates
just to export: revocation can break shipped apps. Cloud-managed keys cannot
be exported locally.
([Apple identity/private-key explanation](https://developer.apple.com/documentation/technotes/tn3161-inside-code-signing-certificates),
[Developer ID revocation impact](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/))

## 4. Create the separate notarization password

Enable two-factor authentication, then at
[account.apple.com](https://account.apple.com/) choose **Sign-In and Security →
App-Specific Passwords → Generate an app-specific password**. Label it “Mist
release notarization”; use it as `APPLE_PASSWORD`, never your primary password.
`APPLE_ID` is that account's sign-in identifier, authorized for the same team.
([Apple app-specific password steps](https://support.apple.com/en-us/102654),
[notarytool authentication](https://developer.apple.com/documentation/security/customizing-the-notarization-workflow))

## 5. Enter the values directly in GitHub

Matching Mist's [workflow](../.github/workflows/release.yml), add these under
**Settings → Environments → release-signing → Environment secrets**; retain
the reviewer protection in [DISTRIBUTION.md](DISTRIBUTION.md).

| Name | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded `.p12`, containing certificate and private key |
| `APPLE_CERTIFICATE_PASSWORD` | Password chosen during `.p12` export |
| `APPLE_SIGNING_IDENTITY` | Exact full certificate Subject Common Name, starting with `Developer ID Application:` |
| `APPLE_ID` | Apple Account sign-in identifier used for notarization |
| `APPLE_PASSWORD` | App-specific password from step 4 |

Add **repository variable** `APPLE_TEAM_ID` under **Settings → Secrets and
variables → Actions → Variables**. Copy the 10-character ID from Apple's
**Membership details** and match the certificate's Subject **Organizational
Unit (OU)**. Copy `APPLE_SIGNING_IDENTITY` from its exact Subject Common Name in
Keychain Access, not an example string.
([Apple Team ID location](https://developer.apple.com/help/glossary/team-id/),
[certificate subject fields](https://developer.apple.com/documentation/technotes/tn3161-inside-code-signing-certificates))

Replace this placeholder to copy the encoded export without printing it:

```sh
base64 -i "/absolute/secure/path/DeveloperIDApplication.p12" | tr -d '\n' | pbcopy
```

Paste only into GitHub's `APPLE_CERTIFICATE` field, then clear the clipboard.
**Base64 is not encryption**; protect it like the original `.p12`. Keep the file
and password securely backed up. This guide contains no credentials and changes
no Apple/GitHub settings.

## Troubleshooting `.cer` import error `-25294`

Apple defines this as `errSecNoSuchKeychain`: the specified keychain could not
be found. It does **not** by itself prove that the certificate is invalid or
that your keychain is corrupt; the underlying cause is still unverified.
([Apple error definition](https://developer.apple.com/documentation/security/errsecnosuchkeychain),
[numeric mapping in Apple's source](https://github.com/apple-oss-distributions/Security/blob/main/base/SecBase.h))

On the CSR-generating Mac, first check that the existing **login** keychain
appears in Keychain Access. Choose **File → Import Items**, select the downloaded
`.cer`, open **Options** if necessary, and inspect **Destination Keychain**.
Explicitly choose **login**, then retry the import. Apple's Developer Technical
Support recommends this route to avoid a wrong default destination and reports
a known import problem with **Local Items**; that is a possible cause, not a
diagnosis of your Mac.
([Apple import steps](https://support.apple.com/guide/keychain-access/import-and-export-keychain-items-kyca35961/mac),
[Apple DTS explanation](https://developer.apple.com/forums/thread/675290))

If **login** is absent or the explicitly targeted import still fails, pause.
These optional commands inspect only keychain names/paths, not stored items:

```sh
security list-keychains -d user
security default-keychain -d user
security login-keychain -d user
```

Do not add `-s`: it changes configuration. Record the macOS version, selected
destination, and exact error for further diagnosis; redact personal paths.
Do not reset/delete keychains, revoke certificates, or override certificate
trust to work around this error.
([Apple's command manual](https://github.com/apple-oss-distributions/Security/blob/main/SecurityTool/macOS/security.1),
[DTS warning about trust overrides](https://developer.apple.com/forums/thread/675290))
