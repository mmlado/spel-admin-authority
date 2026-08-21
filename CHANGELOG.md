# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-08-20

### Changed

- The samples' gated fns take their injected params instead of
  declaring them.

## [0.1.0] - 2026-08-19

First release.

### Added

- Three management instructions: `admin_initialize` creates the Config
  PDA and installs the caller by self-election, `admin_transfer` hands
  the role to a new signer or PDA with candidate validation, and
  `admin_renounce` zeroes the admin permanently with no recovery path by
  design.
- The `#[require_admin]` gate: consumers annotate instructions, the
  framework injects the config and caller params unless already
  declared, and the check rejects a non-admin caller before the handler
  runs. Param names are overridable by kwarg, and an alignment self-test
  keeps the macro's kwargs and the inject metadata from drifting.
- Embedded mode: the `AdminConfig` slot can live inside one of the
  consumer's own accounts at a declared byte offset instead of a
  dedicated PDA. The `#[admin_slot]` field marker keeps the layout
  honest, the consumer's account-creating instruction carries the
  bootstrap so the slot is born initialized in the transaction that
  creates the account, and a program whose slot would ship born
  renounced refuses to compile. Offsets compile into the program and
  never appear in the IDL or a transaction.
- Three consumer samples: the plain integration, a manual integration
  without the marker, and the embedded layout.
- Contract tests, IDL pin tests resolving the sample's own dependency
  graph, dry-run byte compares in CI, and fixture jobs running
  `--locked`.
- Docs packet: CONTEXT.md vocabulary, the account model and authority
  lifecycle, and ADRs for every design decision.

[Unreleased]: https://github.com/mmlado/spel-admin-authority/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.1
[0.1.0]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.0
