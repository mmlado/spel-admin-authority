use std::path::PathBuf;

use spel_framework_core::idl_gen::generate_idl_from_file_with_deps;

#[test]
fn idl_contains_user_instr_and_admin_trio() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src = PathBuf::from(manifest_dir).join("src/main.rs");

    let idl = generate_idl_from_file_with_deps(&src, &[])
        .expect("IDL generation failed");

    let names: Vec<&str> = idl.instructions.iter().map(|i| i.name.as_str()).collect();
    
    assert!(names.contains(&"update_value"), "missing user instr update_value");
    assert!(names.contains(&"admin_initialize"), "missing admin_initialize from path-dep scan");
    assert!(names.contains(&"admin_transfer"), "missing admin_transfer from path-dep scan");
    assert!(names.contains(&"admin_renounce"), "missing admin_renounce from path-dep scan");
}