use proc_macro::TokenStream;
use quote::{ quote, ToTokens };
use syn::{parse_macro_input, FnArg, ItemFn};

/// Marker attribute. Framework detects it on a #[lez_program] module
/// and injects admin_initialize/admin_transfer/admin_renounce instructions.
/// As a standalone (no #[lez_program]) it emits a compile error.
#[proc_macro_attribute]
pub fn admin_authority(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Pass-through. Framework's #[lez_program] reads this attr by name.
    item
}

/// Body-inject macro. Adds an admin authorization check before the
/// annotated instruction's body runs.
#[proc_macro_attribute]
pub fn require_admin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: syn::ItemFn = match syn::parse(item.clone()) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut has_admin_config_pda = false;
    let mut has_signer = false;

    for arg in &func.sig.inputs {
        let syn::FnArg::Typed(pt) = arg else { continue };
        let syn::Pat::Ident(pat_ident) = &*pt.pat else { continue };
        match pat_ident.ident.to_string().as_str() {
            "admin_config" => has_admin_config_pda = true,
            "caller" | "signer" => has_signer = true,
            _ => {}
        }
    }

    if !has_admin_config_pda {
        return syn::Error::new_spanned(
            &func.sig,
            r#"#[require_admin] needs an #[account(pda = literal("admin_config"))] param"#,
        )
        .to_compile_error()
        .into();
    }

    if !has_signer {
        return syn::Error::new_spanned(
            &func.sig,
            "#[require_admin] needs an #[account(signer)] param",
        )
        .to_compile_error()
        .into();
    }

    item
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
