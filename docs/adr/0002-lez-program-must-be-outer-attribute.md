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

Rust proc-macro attributes run outermost-first. `expand_lez_program()` receives the inner attributes as part of the item token stream and scans them for `#[admin_authority]`. If `#[admin_authority]` runs first (the wrong order), it pass-through emits the item without the attribute, and `expand_lez_program()` never sees it. The three injected instructions then go silently missing.

To prevent that silent failure in a security library, `#[admin_authority]` itself emits a `compile_error!` if it ever runs as a standalone macro, pointing the developer at the correct ordering.
