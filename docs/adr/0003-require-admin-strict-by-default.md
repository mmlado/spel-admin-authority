---
status: superseded by ADR-0006
---

# `#[require_admin]` is strict by default; relaxed mode via env var

> Superseded by [ADR-0006](0006-param-injection-and-relaxed-mode.md). The compile-error shape check and the env var described below were replaced. Strict now means release builds require explicitly declared gate params. Relaxed now means dev-only, metadata-driven param injection by the framework.

`#[require_admin]` emits a `compile_error!` if the annotated instruction does not explicitly declare an `#[account(pda = literal("admin_config"))]` param and an `#[account(signer)]` param. Relaxed mode, in which the macro auto-injects `__admin_config` and `__admin_signer` if absent, is opt-in via `SPEL_ADMIN_AUTHORITY_RELAXED=1` at build time. The proc macro reads the variable with `std::env::var` at expansion time.

The macro lives in `admin-authority-macros` (not in `spel-framework-macros`). The framework is library-agnostic and discovers the marker attribute name through `[package.metadata.spel.instruction_attrs]` in admin-authority's `Cargo.toml`. Shape validation logic stays library-owned for the same reason: the rule "admin_config PDA param + signer param" is admin-authority's contract, not the framework's.

## Considered Options

**1. Always strict (no relaxed mode).**
Compile error if either required account is missing. No escape hatch.
Rejected because this creates high friction for prototyping and quick-start use cases, where boilerplate is the main barrier to adoption.

**2. Always relaxed (auto-inject, no strict mode).**
Macro silently injects `__admin_config` and `__admin_signer` whenever they are absent.
Rejected because `admin-authority` is a security primitive. Silent injection means developers may not notice the extra accounts appearing in the IDL, and account naming gives no hint that admin gating is active. Security libraries should surface their requirements loudly.

**3. Per-instruction attribute, `#[require_admin(strict)]` and `#[require_admin(relaxed)]`.**
Each instruction opts into one mode independently.
Rejected because it creates a three-state configuration surface (strict, relaxed, default) with an ambiguous default. It increases the annotation surface for a feature that should be invisible in production code. Mixing modes across instructions in the same module is almost never intentional.

**4. Env var global toggle, strict by default, `SPEL_ADMIN_AUTHORITY_RELAXED=1` to opt in (chosen).**
Two states only. Relaxed is visibly opt-in, appears in build scripts and CI logs, and applies uniformly across all instructions in a build. The annotation stays `#[require_admin]` with no arguments in all cases, so there is no per-instruction configuration surface.
