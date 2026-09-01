# Distribution and GA responsibility

Trigix source code is available under the MIT License. A person or organization may use, modify, publish, distribute, sublicense, or sell copies while preserving the copyright and license notice required by that license. Release maturity and operational responsibility are separate from source-code permission.

`GA` means that a specific distributor considers a specific artifact generally available under that distributor's own support and release policy. It is not a certification automatically inherited from this repository, a source commit, or another distributor's build.

## Distribution classes

| Class | Artifact authority | Required public description | Responsibility |
| --- | --- | --- | --- |
| Official GA | Published by the Trigix maintainers through an official `desktop-v*` GitHub Release | Official Trigix release | Trigix release, security, and support roles for the stated supported boundary |
| Community Build | Built and published by an independent community member or project | Unofficial community build, with the distributor identified | Community distributor |
| Self-managed Distribution | Built or repackaged by an organization for its own users, customers, or managed environment | Organization-managed distribution, not an official Trigix release | That organization |

The same source revision can produce artifacts in more than one class. Signatures, checksums, qualification evidence, support promises, and update trust do not transfer between them.

## What independent distributors may do

Subject to the MIT License and applicable third-party licenses, an independent distributor may:

- deploy the Platform and supporting services;
- modify and build the source;
- sign Windows and macOS packages with its own identity;
- publish checksums, manifests, installers, updates, and release notes;
- define a supported operating-system and architecture matrix;
- call its own identified distribution GA after completing its own gates;
- offer hosting, integration, maintenance, or commercial support.

An independent distributor does not need to wait for Official Desktop GA. Official GA Issues govern only artifacts and support claims made by the Trigix maintainers.

## Distributor obligations

Every distributor is responsible for its own release. At minimum, it should:

1. Preserve the Trigix copyright and MIT License notice and review all third-party licenses included in its artifact.
2. Identify the distributor, source revision, modifications, artifact digest, supported boundary, and support channel.
3. Avoid language, repository layout, signing identity, or download presentation that implies the artifact was published, signed, endorsed, or supported by the Trigix maintainers when it was not.
4. Use its own protected signing identities and release trust. Never reuse a development qualification identity as public trust.
5. Test clean installation, automation, permissions, update, rollback, recovery, uninstall, and accessibility on its claimed matrix.
6. Operate its own vulnerability intake, dependency review, incident response, update, revocation, and customer communication paths.
7. Publish its own privacy notice, product terms, acceptable-use boundary, provider disclosures, retention rules, and legally required notices.
8. Ensure it has authority to distribute licensed media, voices, likenesses, models, credentials, integrations, and other non-MIT material.

The repository's public guides are a reusable baseline, not evidence that an independent artifact passed those gates.

## Signing boundary

For public direct distribution on Windows, use a signing identity trusted for the intended audience and document the verified publisher. Self-issued certificates can be appropriate inside an explicitly managed enterprise trust domain, but they are development or organization trust, not Trigix Official GA trust.

For public direct distribution on macOS, use the distributor's Apple Developer identity, Developer ID certificate, hardened runtime, notarization, and stapling process. A local, ad hoc, or self-issued identity is not an official or publicly notarized identity.

## Required labels and release record

A Community Build release page should state:

> This is an unofficial Community Build of Trigix. It is built, signed, distributed, updated, and supported by [Distributor]. It is not an official Trigix release. Source revision: [commit]. Modifications: [link].

A Self-managed Distribution should state:

> This Trigix distribution is managed by [Organization]. [Organization] is responsible for its signing identity, qualification, updates, security response, privacy compliance, and support. It is not an official Trigix release.

The release record should also contain artifact hashes, publisher identity, supported systems, known limitations, verification instructions, source offer or source link as applicable, third-party notices, security channel, support channel, and update or rollback instructions.

## Official naming and downloads

Only artifacts attached by the Trigix maintainers to an official stable `desktop-v*` release are presented on the Trigix website as Official Desktop GA downloads. Other builds may accurately describe their relationship to Trigix source, but must identify their distributor and must not claim Official GA status or Trigix maintainer support.

Questions about a third-party build go to its distributor. The Trigix public support channel may accept source-level defect reports that reproduce on an official supported source or artifact, but it does not assume installation, signing, update, incident, privacy, or customer-support responsibility for independent distributions.
