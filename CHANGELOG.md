# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4] - 2026-09-17

### Changed

- `#[instruction]` is re-exported from `spel-framework` rather than shipped as a shim in `admin-authority-macros`. The framework's own `#[instruction]` strips the `#[account(...)]` helper attrs when it expands outside `#[lez_program]`, so the hand-written copy had nothing left to do (logos-co/spel#271).
- Pin `spel-framework` to `logos-co/spel` at d0bb659, the commit that
  merged that fix.
- Pin `spel-authority` by its `v0.1.2` tag.

## [0.1.3] - 2026-09-11

### Changed

- Pin `spel-framework` to upstream `logos-co/spel`, at the commit that
  merged the extension mechanism. The samples move to LEZ `v0.2.4` with
  it.
- Pin `spel-authority` by its `v0.1.1` tag.

## [0.1.2] - 2026-08-24

### Changed

- Pin `spel-authority` by its `v0.1.0` tag.

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

[Unreleased]: https://github.com/mmlado/spel-admin-authority/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.4
[0.1.3]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.3
[0.1.2]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.2
[0.1.1]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.1
[0.1.0]: https://github.com/mmlado/spel-admin-authority/releases/tag/v0.1.0
