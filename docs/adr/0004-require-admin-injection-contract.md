---
status: accepted
---

# The require_admin check is injected by re-expansion

The runtime admin check (decode the Config PDA, then `assert_admin`) is injected by re-expanding the `#[require_admin]` proc-macro on the handler that `#[lez_program]` emits. The framework does not splice it in and does not strip the attribute.

## Why

A gate attribute is an ordinary proc-macro. Left on the emitted handler it re-expands, prepends its check, and consumes itself. So the framework does nothing for the injection. It must not list gate attributes for stripping, because stripping one removes the check and lets a non-admin caller through. An earlier M1 iteration listed `require_admin` for stripping, which suppressed the re-expansion. Removing it from the strip list lets the check land. Verified by `cargo expand` on `admin-authority-sample`, where the decode and `assert_admin` prologue lands at the top of the handler and the `#[require_admin]` attribute is gone.

`#[require_admin]` reads its target parameter names from attribute arguments with conventional defaults, `config = admin_config` and `signer = caller`. A consumer whose params are named differently passes the args explicitly. The macro never reads `#[account]`. That attribute belongs to the framework, which reads it for the validator and IDL and strips all of it once.

## Considered options

1. Framework inline codegen, the proc-macro left as a stub. Rejected. It forces the framework to know each extension's prologue and duplicates the injection logic.
2. The macro reads and strips `#[account]` itself. Rejected. It breaks when two gates sit on one function and leaves unrelated account params unowned. `#[account]` has one owner, the framework.

## Consequences

- No framework code is needed for the check. Self-removing macros do the work.
- How the gate accounts are provided, and how they are declared or injected, is a separate decision. It will be recorded in its own ADR when it is built.
