# Windows Desktop release qualification

## Supported release envelope

Official Trigix Desktop Windows releases target x64 Windows 11 Pro or Enterprise 24H2 and 25H2. Windows 10, Windows ARM64, Windows Sandbox, Windows containers, Session 0, locked or disconnected sessions, and cross-user automation are outside this release boundary.

The public artifact is an NSIS installer built with the application and `desktop-automation-host` from one reviewed source revision. The installer, application, and Host must carry the same production Authenticode identity and a verifiable public timestamp.

## Required protected configuration

Create a GitHub environment named `desktop-production`, require production reviewer approval, restrict deployment to `desktop-v*` tags, and configure these environment secrets:

- `WINDOWS_CERTIFICATE`: base64-encoded production code-signing PKCS#12 archive;
- `WINDOWS_CERTIFICATE_PASSWORD`: PKCS#12 password;
- `WINDOWS_SIGNING_SUBJECT`: exact certificate subject expected on every signed executable;
- `WINDOWS_TIMESTAMP_URL`: approved HTTP or HTTPS Authenticode timestamp endpoint without embedded credentials; prefer HTTPS when the provider supports it.

Use a hardware- or managed-service-protected production identity for an Official release. The PKCS#12 input is an integration boundary for the protected runner; it must not be stored in repository secrets if organizational policy requires a managed signing service. In that case, replace only the protected import/sign step with the service integration and preserve the same verification and evidence contract.

Do not place certificate material, passwords, timestamp credentials, private report URLs, or customer information in source, workflow inputs, logs, issues, or release assets. Rotate any value exposed outside the protected environment.

## Build and publish

1. Confirm blocking CI, dependency review, the independent penetration-test disposition, and the exact supported-client test plan are approved for the release revision.
2. Set `apps/desktop/src-tauri/tauri.conf.json` to the stable `MAJOR.MINOR.PATCH` version and merge it to the default branch.
3. Create a signed annotated tag named `desktop-vMAJOR.MINOR.PATCH` on the current default-branch revision and push it.
4. Approve the `Release Desktop Windows` deployment in the protected environment.
5. The workflow imports the protected identity into the ephemeral current-user certificate store, builds the NSIS installer, rejects an invalid signer or missing timestamp before execution, installs into an isolated directory, and verifies the packaged application and Host.
6. The workflow emits an installed-payload SPDX SBOM, SHA-256 checksum, Sigstore provenance bundle, and content-free signing evidence. It retains the evidence artifact for 90 days and publishes immutable download assets to the matching GitHub Release.
7. Confirm the release assets, downloaded digest, publisher subject, public timestamp, SBOM, and attestation match the protected evidence before client qualification begins.

Manual dispatch rebuilds only an existing immutable release tag. Asset upload intentionally fails when an asset with the same name already exists. Delete-and-replace is not an approved release repair; publish a new version after review.

## Clean-client qualification matrix

Record a result for both supported Windows versions on physical or organization-controlled virtual Windows 11 Pro or Enterprise x64 devices:

| Journey | 24H2 x64 | 25H2 x64 |
| --- | --- | --- |
| Download, checksum, publisher, timestamp, and malware verification | Required | Required |
| Clean install, first launch, pair, reconnect, and uninstall | Required | Required |
| Upgrade from the previous supported release and recovery after interruption | Required | Required |
| Inspection, focus, semantic invoke, verified Unicode entry, and protected-control rejection | Required | Required |
| Approval, lease expiry, cancellation, Host crash recovery, halt, and signed rollback | Required | Required |
| Voice proposal, microphone stop, avatar fallback, and audit correlation | Required | Required |

Record exact OS build, device class, artifact SHA-256, publisher subject, timestamp result, malware/EDR tool and definition version, operator, start/end time, failures, retest references, and final disposition. CI on Windows Server is blocking engineering qualification but does not replace these client runs.

## Release gates

Production publication or stable promotion remains blocked when any of these conditions is true:

- the protected environment, reviewer rule, production identity, or timestamp service is unavailable;
- the tag does not match the product version and current default-branch revision;
- installer, application, or Host Authenticode validation fails or identifies another publisher;
- checksum, installed-payload SBOM, provenance, dependency review, or immutable evidence is missing;
- either supported Windows client matrix is incomplete;
- malware or EDR assessment is stale or unresolved;
- the independent penetration test has an unresolved critical or high finding;
- closed-beta convergence, halt, incident, and signed rollback evidence is incomplete;
- the accountable release owner has not approved the exact digest.

Self-signed CI packages remain development qualification artifacts. They cannot satisfy this production gate and must never be presented as Official Trigix installers.
