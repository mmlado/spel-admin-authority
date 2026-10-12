fn main() {
    // The genesis key is compiled in, so a changed key must rebuild this crate.
    println!("cargo:rerun-if-env-changed=SPEL_ADMIN_GENESIS");
}
