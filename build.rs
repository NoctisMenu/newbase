fn main() {
    // The overlay's version banner is baked in at compile time from COMMIT_HASH
    // via `option_env!`. Without this, cargo won't rebuild newbase when the
    // variable changes, so the banner could go stale across builds.
    println!("cargo:rerun-if-env-changed=COMMIT_HASH");
}
