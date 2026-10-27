use vae::raw::{Serializable, functions};

#[test]
fn alias_reexports_vaegram() {
    assert_eq!(functions::help::GetConfig {}.to_bytes(), [0x6b, 0x18, 0xf9, 0xc4]);
}
