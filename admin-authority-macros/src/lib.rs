//! Proc-macro companion crate for `admin-authority`.
//!
//! Provides `#[admin_authority]` (the module marker the framework discovers
//! by name), `#[require_admin]` (prepends the runtime admin check to a gated
//! instruction by re-expansion), and an internal `#[instruction]` shim that
//! strips `#[account(...)]` helper attrs so the library compiles standalone.
//!
//! Attribute macros must live in a `proc-macro = true` crate, which cannot
//! export runtime items. Consumers never depend on this crate directly, the
//! `admin-authority` library re-exports everything.

#![warn(missing_docs)]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, FnArg, ItemFn, MetaNameValue, Token, parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

/// Marker attribute. Framework detects it on a #[lez_program] module
/// and merges the admin instructions into the dispatcher and IDL.
///
/// Attributes expand top to bottom, so if `#[lez_program]` is still
/// visible on the module when this macro runs, the marker was written
/// above it. The framework would never see the marker, silently
/// skipping discovery, so that placement is a hard error.
#[proc_macro_attribute]
pub fn admin_authority(_attr: TokenStream, item: TokenStream) -> TokenStream {
    if let Ok(module) = syn::parse::<syn::ItemMod>(item.clone())
        && let Some(err) = misplaced_above_lez_program(&module)
    {
        return err.to_compile_error().into();
    }
    item
}

/// The marker was written above `#[lez_program]` when that attribute is
/// still visible on the module the marker expands on.
fn misplaced_above_lez_program(module: &syn::ItemMod) -> Option<syn::Error> {
    if module
        .attrs
        .iter()
        .any(|a| a.path().is_ident("lez_program"))
    {
        Some(syn::Error::new_spanned(
            &module.ident,
            "#[admin_authority] must come after #[lez_program]: a marker above \
            it expands first and is invisible to the framework",
        ))
    } else {
        None
    }
}

/// Embedded-mode initializer: injects the bootstrap into the consumer's
/// account-creating instruction. The framework stamps `admin_config`
/// and `offset`; the consumer may name their signer with `caller = x`.
/// The named embedding param must be `#[account(init)]` in this same
/// instruction: `init` is the only fresh-account guarantee, and a
/// bootstrap against an existing account would be a takeover.
#[proc_macro_attribute]
pub fn admin_initialize(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(
        attr with Punctuated::<MetaNameValue, Token![,]>::parse_terminated
    );

    let mut config_ident = format_ident!("admin_config");
    let mut caller_ident = format_ident!("caller");
    let mut offset_expr: Option<Expr> = None;

    for pair in args {
        let Some(key) = pair.path.get_ident().map(|i| i.to_string()) else {
            return syn::Error::new_spanned(&pair.path, "expected an identifier key")
                .to_compile_error()
                .into();
        };
        match key.as_str() {
            "offset" => offset_expr = Some(pair.value),
            "admin_config" | "caller" => {
                let Expr::Path(p) = &pair.value else {
                    return syn::Error::new_spanned(&pair.value, "expected a bare parameter name")
                        .to_compile_error()
                        .into();
                };
                let ident = p.path.get_ident().cloned().expect("bare ident");
                if key == "admin_config" {
                    config_ident = ident;
                } else {
                    caller_ident = ident;
                }
            }
            other => {
                return syn::Error::new_spanned(
                    &pair.path,
                    format!("unknown key `{other}`; expected `admin_config`, `caller` or `offset`"),
                )
                .to_compile_error()
                .into();
            }
        };
    }

    let Some(offset) = offset_expr else {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[admin_initialize] is embedded-mode only and its location is \
            framework-stamped; dedicated mode provides the admin_initialize \
            instruction instead",
        )
        .to_compile_error()
        .into();
    };

    let mut func: syn::ItemFn = parse_macro_input!(item as ItemFn);

    let stmts = &func.block.stmts;
    let (body, tail) = stmts.split_at(stmts.len().saturating_sub(1));
    let bootstrap: syn::Stmt = parse_quote! {
        ::admin_authority::AdminConfig::bootstrap_at(
            &mut #config_ident,
            #offset,
            ::admin_authority::AdminCandidate::Signer,
            &#caller_ident,
        )?;
    };
    let new_stmts: Vec<syn::Stmt> = body
        .iter()
        .cloned()
        .chain(std::iter::once(bootstrap))
        .chain(tail.iter().cloned())
        .collect();
    func.block.stmts = new_stmts;

    quote!(#func).into()
}

/// Body-inject macro. Prepends an admin authorization check (decode the
/// Config PDA + `assert_admin`) to the annotated instruction's body, so a
/// non-Admin caller is rejected before the handler's own logic runs.
///
/// Target params are supplied as attribute arguments, defaulting to the
/// conventional names:
/// - `config` — the Config PDA account param (default `admin_config`)
/// - `signer` — the signing caller param (default `caller`)
///
/// ```ignore
/// #[require_admin]                                  // uses defaults
/// #[require_admin(config = my_cfg, signer = owner)] // overrides both
/// ```
///
/// The macro never reads `#[account(...)]`; that attribute belongs solely to
/// the framework. In the `#[lez_program]` path the framework resolves and
/// injects these args during expansion (see ADR-0004).
#[proc_macro_attribute]
pub fn require_admin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(
        attr with Punctuated::<MetaNameValue, Token![,]>::parse_terminated
    );

    let mut config_ident = format_ident!("admin_config");
    let mut caller_ident = format_ident!("caller");
    let mut offset: syn::Expr = syn::parse_quote!(0);

    for pair in args {
        if pair.path.is_ident("offset") {
            offset = pair.value.clone();
            continue;
        }
        let value_ident = match &pair.value {
            Expr::Path(p) if p.path.get_ident().is_some() => p.path.get_ident().unwrap().clone(),
            other => {
                return syn::Error::new_spanned(other, "expected a bare parameter name")
                    .to_compile_error()
                    .into();
            }
        };

        if pair.path.is_ident("admin_config") {
            config_ident = value_ident;
        } else if pair.path.is_ident("caller") {
            caller_ident = value_ident;
        } else {
            return syn::Error::new_spanned(
                &pair.path,
                "unknown key; expected `admin_config` or `caller` or `offset`",
            )
            .to_compile_error()
            .into();
        }
    }

    let mut func: syn::ItemFn = parse_macro_input!(item as ItemFn);

    let prologue: syn::Stmt = parse_quote! {{
        let __admin_cfg = ::admin_authority::AdminConfig::from_account_at(&#config_ident, #offset)?;
        __admin_cfg.assert_admin(&#caller_ident)?;
    }};

    func.block.stmts.insert(0, prologue);
    quote!(#func).into()
}

/// No-op `#[instruction]` for path-dep-scanned admin fns. Strips
/// `#[account(...)]` helper attrs from params so rustc accepts the
/// admin-authority crate compile. The path-dep scanner reads raw source
/// via `syn::parse_file` and sees the `#[account(...)]` attrs intact.
#[proc_macro_attribute]
pub fn instruction(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(item as ItemFn);
    for arg in &mut func.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            pt.attrs.retain(|a| !a.path().is_ident("account"));
        }
    }
    quote!(#func).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_above_lez_program_is_rejected() {
        // #[lez_program] still visible on the module means this marker
        // expanded first, so it sits above and the framework never sees it.
        let module: syn::ItemMod = parse_quote! {
            #[lez_program]
            mod program {}
        };
        let err = misplaced_above_lez_program(&module).expect("must reject");
        assert!(
            err.to_string().contains("must come after #[lez_program]"),
            "unexpected message: {err}"
        );
    }

    #[test]
    fn marker_below_lez_program_passes() {
        // Correct placement: by the time a below-marker could expand,
        // #[lez_program] has consumed itself and is no longer on the module.
        let module: syn::ItemMod = parse_quote! {
            #[doc = "no lez_program attr here"]
            mod program {}
        };
        assert!(misplaced_above_lez_program(&module).is_none());
    }
}
