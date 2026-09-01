# Incident and release communication templates

These templates keep public updates factual and free of credentials, private infrastructure, customer content, unverified cause, and speculative recovery times. Replace every bracketed field before use. Approval and exercised contact paths are production release gates outside this repository.

## Initial incident notice

**Trigix [component] incident under investigation**

We are investigating [confirmed user-visible impact] affecting [supported scope] since [UTC time]. [Safe workaround or "No safe workaround is confirmed yet."]. Desktop users should [stop, remain on the installed version, or other bounded action]. We will update this notice by [UTC time or condition]. Security-sensitive details will be coordinated privately.

## Progress update

**Trigix [component] incident update**

Confirmed impact: [facts]. Affected versions or release digests: [identifiers]. Current containment: [halt, feature disablement, revocation, or none]. User action: [bounded instruction]. We have not confirmed [important unknown]. Next update: [UTC time or condition].

## Recovery notice

**Trigix [component] service restored**

The user-visible impact ended at [UTC time]. Recovery requires [version, verified artifact, restart, re-pair, or no action]. We verified [supported matrix and evidence]. Users still affected should follow [public troubleshooting link] and report [safe identifiers]. A follow-up will address [known remaining work] by [date or milestone].

## Security release notice

**Security update available for Trigix [component]**

Version [version] addresses [bounded impact description]. Affected versions: [range]. Users should download only from [immutable release URL], verify [checksum and publisher identity], and complete [upgrade, key rotation, re-pair, or other action] by [deadline]. Advisory: [URL]. Do not post exploit details or credentials in public issues.

## Desktop launch notice

**Trigix Desktop [version] is available**

Supported systems: [Windows versions and architecture], [macOS versions and architectures]. Download: [immutable release URL]. SHA-256 and publisher identity: [release evidence URL]. Known limitations: [URL]. Installation and verification: [URL]. Support: [URL]. Private security reports: [URL].

## Rollout halt

**Trigix Desktop [version] rollout paused**

Promotion paused at [UTC time] because [confirmed bounded reason]. Users who have not installed should remain on [safe version]. Users already on [affected version] should [safe action]. Do not install from mirrors until [verification condition]. The next update will be published by [UTC time or condition].

## Rollback instruction

**Approved rollback for Trigix Desktop [version]**

Use only the signed rollback authorized through [manifest sequence and immutable URL]. It applies only to devices currently on [exact version]. Verify [checksum, publisher, manifest, and release target] before proceeding. Do not manually reinstall an older package or edit update metadata. After rollback, confirm [pairing, recovery, fleet, and audit checks].

## Approval checklist

- [ ] Scope, time, versions, digests, and user impact are confirmed
- [ ] User action is safe, tested, and limited to the affected boundary
- [ ] Release owner approved artifact and rollback wording
- [ ] Product owner approved public impact wording
- [ ] Security owner approved security and disclosure wording when applicable
- [ ] Support channel and next-update commitment are staffed
- [ ] Links point to immutable public records and contain no private data
