//! Generates foreign-language bindings, e.g.
//! `cargo run -p escrituras-ffi --bin uniffi-bindgen -- generate --library <lib> --language swift --out-dir <dir>`

fn main() {
    uniffi::uniffi_bindgen_main()
}
