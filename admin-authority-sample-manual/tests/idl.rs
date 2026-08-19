use std::path::PathBuf;

use spel_framework_core::idl::IdlSeed;
use spel_framework_core::idl_gen::generate_idl_from_file_with_deps;

#[test]
fn idl_contains_manual_instructions_and_no_injected_trio() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src = PathBuf::from(manifest_dir).join("src/main.rs");

    let idl = generate_idl_from_file_with_deps(&src, &[]).expect("IDL generation failed");

    let names: Vec<&str> = idl.instructions.iter().map(|i| i.name.as_str()).collect();

    assert!(
        names.contains(&"initialize"),
        "missing user instr initialize"
    );
    assert!(
        names.contains(&"update_value"),
        "missing user instr update_value"
    );
    assert!(
        names.contains(&"admin_transfer"),
        "missing user instr admin_transfer"
    );
    assert!(
        names.contains(&"admin_renounce"),
        "missing user instr admin_renounce"
    );
    assert!(
        !names.contains(&"admin_initialize"),
        "admin_initialize found"
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
}
