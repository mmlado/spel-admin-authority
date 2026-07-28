use std::path::PathBuf;

use spel_framework_core::idl::IdlSeed;
use spel_framework_core::idl_gen::generate_idl_from_file_with_deps;

#[test]
fn idl_shows_embedded_surface_and_no_initializer_or_offset() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src = PathBuf::from(manifest_dir).join("src/main.rs");

    let deps = [
        PathBuf::from(manifest_dir)
            .join("../admin-authority")
            .canonicalize()
            .expect("admin-authority dir"),
        PathBuf::from(manifest_dir)
            .join("../../spel-authority")
            .canonicalize()
            .expect("spel-authority dir"),
    ];
    let idl = generate_idl_from_file_with_deps(&src, &deps).expect("IDL generation failed");

    let names: Vec<&str> = idl.instructions.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "initialize",
            "update_value",
            "poke",
            "admin_transfer",
            "admin_renounce"
        ],
        "embedded surface drifted"
    );
    assert!(
        !names.contains(&"admin_initialize"),
        "embedded mode must not emit the initializer"
    );

    // The slot is born initialized by the consumer's initialize; the
    // dedicated admin_config PDA must appear nowhere.
    for ix in &idl.instructions {
        assert!(
            ix.accounts.iter().all(|a| a.name != "admin_config"),
            "`{}` still references the dedicated admin_config PDA",
            ix.name
        );
        assert!(
            ix.args.iter().all(|a| a.name != "offset"),
            "`{}` leaks the bound offset into the ABI",
            ix.name
        );
    }

    // Gated fn that declares the embedding account: declared param wins,
    // caller is injected.
    let update_value = idl
        .instructions
        .iter()
        .find(|i| i.name == "update_value")
        .unwrap();
    let config = update_value
        .accounts
        .iter()
        .find(|a| a.name == "config")
        .expect("update_value must carry the embedding account");
    let pda = config.pda.as_ref().expect("config must be a PDA account");
    assert!(
        matches!(&pda.seeds[..], [IdlSeed::Const { value }] if value == "prog_config"),
        "embedding account seed drifted: {:?}",
        pda.seeds
    );
    assert!(
        update_value.accounts.iter().any(|a| a.name == "caller" && a.signer),
        "caller must be injected as a signer"
    );

    // Gated fn that declares nothing: both gate accounts are synthesized,
    // the embedding account PDA-verified with the canonical constraint.
    let poke = idl.instructions.iter().find(|i| i.name == "poke").unwrap();
    let poke_config = poke
        .accounts
        .iter()
        .find(|a| a.name == "config")
        .expect("poke must get the embedding account injected");
    let poke_pda = poke_config
        .pda
        .as_ref()
        .expect("injected embedding account must be PDA-verified");
    assert!(
        matches!(&poke_pda.seeds[..], [IdlSeed::Const { value }] if value == "prog_config"),
        "injected embedding account seed drifted: {:?}",
        poke_pda.seeds
    );

    // Discovered management fns: role param substituted to the consumer
    // account, the offset stripped from the args.
    let transfer = idl
        .instructions
        .iter()
        .find(|i| i.name == "admin_transfer")
        .unwrap();
    let transfer_config = transfer
        .accounts
        .iter()
        .find(|a| a.name == "config")
        .expect("admin_transfer must target the embedding account");
    let transfer_pda = transfer_config
        .pda
        .as_ref()
        .expect("substituted param must keep a PDA constraint");
    assert!(
        matches!(&transfer_pda.seeds[..], [IdlSeed::Const { value }] if value == "prog_config"),
        "substituted constraint drifted: {:?}",
        transfer_pda.seeds
    );
    let transfer_args: Vec<&str> = transfer.args.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        transfer_args,
        vec!["new_admin"],
        "admin_transfer args drifted"
    );

    let renounce = idl
        .instructions
        .iter()
        .find(|i| i.name == "admin_renounce")
        .unwrap();
    assert!(
        renounce.accounts.iter().any(|a| a.name == "config"),
        "admin_renounce must target the embedding account"
    );
    assert!(
        renounce.args.is_empty(),
        "admin_renounce must take no args, found: {:?}",
        renounce.args.iter().map(|a| a.name.as_str()).collect::<Vec<_>>()
    );

    // The candidate enum still resolves through the alias.
    assert!(
        idl.types.iter().any(|t| t.name == "AdminCandidate"),
        "types array must carry AdminCandidate"
    );
}
