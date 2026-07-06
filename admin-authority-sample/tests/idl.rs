use std::path::PathBuf;

use spel_framework_core::idl::IdlSeed;
use spel_framework_core::idl_gen::generate_idl_from_file_with_deps;

#[test]
fn idl_contains_user_instr_and_admin_trio() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src = PathBuf::from(manifest_dir).join("src/main.rs");

    let idl = generate_idl_from_file_with_deps(&src, &[]).expect("IDL generation failed");

    let names: Vec<&str> = idl.instructions.iter().map(|i| i.name.as_str()).collect();

    assert!(
        names.contains(&"update_value"),
        "missing user instr update_value"
    );
    assert!(
        names.contains(&"admin_initialize"),
        "missing admin_initialize from path-dep scan"
    );
    assert!(
        names.contains(&"admin_transfer"),
        "missing admin_transfer from path-dep scan"
    );
    assert!(
        names.contains(&"admin_renounce"),
        "missing admin_renounce from path-dep scan"
    );

    let update_value = idl
        .instructions
        .iter()
        .find(|i| i.name == "update_value")
        .unwrap();
    let admin_cfg = update_value
        .accounts
        .iter()
        .find(|a| a.name == "admin_config")
        .expect("update_value must declare the admin_config account");
    let pda = admin_cfg
        .pda
        .as_ref()
        .expect("admin_config must be a PDA account");
    assert!(
        matches!(&pda.seeds[..], [IdlSeed::Const { value }] if value == "admin_config"),
        "admin_config PDA seed changed: {:?}",
        pda.seeds
    );

    // admin_initialize self-elects the caller (ADR-0005): two accounts, no
    // candidate arg. Fails if a candidate param is ever reintroduced.
    let admin_init = idl
        .instructions
        .iter()
        .find(|i| i.name == "admin_initialize")
        .unwrap();
    assert_eq!(
        admin_init.accounts.len(),
        2,
        "admin_initialize must have exactly config + caller: {:?}",
        admin_init
            .accounts
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
    );
    let config = &admin_init.accounts[0];
    assert_eq!(config.name, "config");
    assert!(config.init, "config must be init");
    let config_pda = config.pda.as_ref().expect("config must be a PDA account");
    assert!(
        matches!(&config_pda.seeds[..], [IdlSeed::Const { value }] if value == "admin_config"),
        "config PDA seed changed: {:?}",
        config_pda.seeds
    );
    let caller = &admin_init.accounts[1];
    assert_eq!(caller.name, "caller");
    assert!(caller.signer, "caller must be a signer");
    assert!(
        admin_init.args.is_empty(),
        "admin_initialize must take no args, found: {:?}",
        admin_init
            .args
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
    );
}
