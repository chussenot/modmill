//! A minimal ProTracker module built byte-for-byte in Rust, so the test
//! suite has one fixture that never depends on `fixtures/` or any
//! download — CI can run with the `fixtures/` directory deleted and this
//! still passes.

/// One sample (silence, 2 bytes), one pattern (all-empty rows), no effects.
pub fn tiny_mod_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0u8; 20]); // song name
    // sample 1: name(22) + length_words(2) + finetune(1) + volume(1)
    //           + loop_start_words(2) + loop_len_words(2) = 30 bytes
    out.extend_from_slice(&[0u8; 22]);
    out.extend_from_slice(&1u16.to_be_bytes()); // length: 1 word = 2 bytes
    out.push(0); // finetune
    out.push(64); // volume
    out.extend_from_slice(&0u16.to_be_bytes()); // loop start
    out.extend_from_slice(&1u16.to_be_bytes()); // loop length (1 = no loop)
    // samples 2..=31: all-empty headers
    for _ in 0..30 {
        out.extend_from_slice(&[0u8; 30]);
    }
    out.push(1); // song length: 1 order entry
    out.push(0x7F); // restart position
    let mut order = [0u8; 128];
    order[0] = 0;
    out.extend_from_slice(&order);
    out.extend_from_slice(b"M.K.");
    out.extend_from_slice(&[0u8; 1024]); // one empty pattern (64 rows * 4 ch * 4 bytes)
    out.extend_from_slice(&[0u8, 0]); // 1 sample word of PCM data (silence)
    out
}

#[test]
fn tiny_mod_has_expected_shape() {
    let bytes = tiny_mod_bytes();
    // header: 20 (name) + 31*30 (samples) + 1 (len) + 1 (restart) + 128 (order) + 4 (tag)
    assert_eq!(20 + 31 * 30 + 1 + 1 + 128 + 4, 1084);
    assert_eq!(&bytes[1080..1084], b"M.K.");
    assert_eq!(bytes.len(), 1084 + 1024 + 2);
}
