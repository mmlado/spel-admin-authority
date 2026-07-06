---
status: accepted
---

# Gate accounts are injected from metadata when a gated instruction omits them

A `#[require_admin]` handler needs the `admin_config` and `caller` params. Typing them on every gated instruction is friction. The framework therefore injects missing gate params at parse time, driven by metadata the extension declares. Injection is always active and skip-if-declared. Whether release builds should instead require explicit declaration (a `#[cfg(not(debug_assertions))] compile_error!` was prototyped and works) is deliberately left open for maintainer feedback on the PR. This supersedes ADR-0003's `SPEL_ADMIN_AUTHORITY_RELAXED` env var design.

## The mechanism

The extension declares what its gate needs in its own `Cargo.toml`:

```toml
[[package.metadata.spel.inject]]
wrapper = "require_admin"

  [[package.metadata.spel.inject.account]]
  name = "admin_config"
  seed = { const = "admin_config" }

  [[package.metadata.spel.inject.account]]
  name = "caller"
  signer = true
```

During `#[lez_program]` expansion, before `parse_instruction` runs, the framework checks each instruction carrying the wrapper attribute. For each declared inject account missing from the signature, it synthesizes the param with its `#[account(...)]` constraints and prepends it. Injection also runs in the `idl_gen.rs` source scanner so both IDL producers stay identical.

The framework reads only metadata. It knows nothing about admin-authority. Any extension can declare an inject block.

## Rules

- **Skip-if-declared.** A param that exists is never injected. A fully declared handler compiles identically in dev and release, so a released program can always be run and upgraded in dev.
- **Injection produces exactly what the declaration would.** Same param, same constraints, same position rules. The dev-built IDL therefore matches what the release-declared IDL will be.
- **ProgramContext stays first.** Injected params go after a leading `ProgramContext` if present.
- **Injected params are prepended** in the metadata's declaration order, before the instruction's own accounts. This ordering is part of the instruction's ABI and is documented in the IDL.
- **Both producers or neither.** The compile-time expansion and the source scanner run the same injection. A change to one without the other is a bug.
- **Consumer-authored fns only, in effect.** An extension's own gated instructions must declare their params or the extension crate itself would not compile, so injection into discovered cross-crate fns is always a skip-if-declared no-op. That is also load-bearing: injecting there would break the arity of the cross-crate dispatch call against the dep's compiled signature.
- **Specs activate with their extension's marker.** A dep's inject block is collected only when its `extension_attr` is on the consumer's module, the same opt-in that activates its instructions.

## The release question, left open on purpose

An earlier draft gated injection to dev builds: the expansion also emitted `#[cfg(not(debug_assertions))] compile_error!(...)`, so release builds demanded explicit declaration. The mechanism works (verified in the relaxed-mode prototype: the cfg resolves in the consumer's own build, the macro never detects the profile itself). It is not shipped in v1 because always-on is simpler, the injected params are identical to declared ones anyway, and the maintainers are better placed to pick the default posture. The PR asks them. Caveat recorded for that discussion: `debug_assertions` tracks the opt profile, not a semantic production flag; a dedicated cargo feature is the explicit alternative.

## Considered options

1. Always require declaration, no injection. Safest and simplest. Rejected as the only mode because typing the gate params on every gated instruction was the recurring friction, and extensions whose gates apply to many instructions multiply it.
2. Env-var relaxed mode (ADR-0003). Superseded. Global, invisible in the build config, and reads the environment at expansion time.
3. Injection in the compile-time producer only. Rejected. The source scanner would emit a different IDL, and producer divergence already bit this project once.
4. Dev-only injection behind a profile gate. Prototyped, works, deferred to maintainer feedback.
5. Metadata-driven injection, always active, skip-if-declared. Chosen.

## Consequences

- New framework surface on the fork: an `inject` metadata reader in `spel-framework-core::extension`, the injection pass in `expand_lez_program` and `expand_generate_idl`, and its mirror in `idl_gen.rs`.
- admin-authority declares the inject block above. One sample keeps fully declared params (showing the explicit style), leaving injection demonstrable in the other or in tests.
- ADR-0003 is superseded. Declaration is a style choice, not a mode. The dev/release split is an open question on the PR.
