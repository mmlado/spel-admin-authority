# `#[lez_program]` must be the outer (first) attribute

`#[admin_authority]` must always be placed *inside* `#[lez_program]`, not outside it:

```rust
// correct
#[lez_program]
#[admin_authority]
mod my_program { ... }

// wrong, silent failure
#[admin_authority]
#[lez_program]
mod my_program { ... }
```

Rust proc-macro attributes run outermost-first. `expand_lez_program()` receives the inner attributes as part of the item token stream and uses them to drive its generic extension scanner (matching attr names against path-deps' `[package.metadata.spel.extension_attr]`). If `#[admin_authority]` runs first (the wrong order), the auto-stripped attribute is consumed by the admin-authority-macros pass-through, never reaches `expand_lez_program()`, and the path-dep scan trigger goes silently missing, no admin instructions injected, no error.

Detection options considered:

1. **Compile_error from `#[admin_authority]` when standalone**, original plan. Now infeasible: the macro is a generic pass-through in admin-authority-macros, with no awareness of whether `#[lez_program]` ran outer or inner.
2. **Framework warning when an extension attr is seen at the wrong scope**, possible enhancement: `#[lez_program]` could check whether the consumer's outer attribute list mentions any extension attr name and warn. Not implemented yet.
3. **Documentation + sample** (current), the documented integration pattern always shows `#[lez_program]` outer. Sample programs follow this ordering. Acceptable for now; revisit if user reports indicate confusion.
