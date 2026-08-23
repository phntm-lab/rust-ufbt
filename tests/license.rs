use md5::{Digest, Md5};

const GPL3_LEN: usize = 35149;
const GPL3_MD5: &str = "1ebbd3e34237af26da5dc08a4e440464";

fn license_text() -> String {
    include_str!("../LICENSE").replace("\r\n", "\n")
}

fn md5_hex(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn license_is_verbatim_gpl3() {
    let text = license_text();
    assert_eq!(text.len(), GPL3_LEN);
    assert_eq!(md5_hex(text.as_bytes()), GPL3_MD5);
}

#[test]
fn license_header_names_gpl_version_3() {
    let text = license_text();
    let mut lines = text.lines();
    assert_eq!(lines.next().unwrap().trim(), "GNU GENERAL PUBLIC LICENSE");
    assert_eq!(lines.next().unwrap().trim(), "Version 3, 29 June 2007");
}

#[test]
fn manifest_declares_gpl3_only() {
    assert_eq!(env!("CARGO_PKG_LICENSE"), "GPL-3.0-only");
}
