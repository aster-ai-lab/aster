use super::*;

const DATA: &[u8] = include_bytes!("../testdata/node_v20.11.0_SHASUMS256.txt");
const SIGNATURE: &[u8] = include_bytes!("../testdata/node_v20.11.0_SHASUMS256.txt.sig");

#[test]
fn official_historical_node_signature_verifies_and_tampered_bytes_do_not() {
    verify_node(DATA, SIGNATURE).unwrap();
    let mut changed = DATA.to_vec();
    changed.push(b'!');
    assert!(matches!(
        verify_node(&changed, SIGNATURE),
        Err(NodeSignatureError::InvalidSignature)
    ));
    assert!(verify_node(DATA, &SIGNATURE[..SIGNATURE.len() / 2]).is_err());
}

#[test]
fn all_pinned_public_key_blocks_are_parsed_and_inputs_are_bounded() {
    let input = include_str!("../node.asc");
    let keys = public_keys(input).unwrap();
    assert_eq!(
        keys.len(),
        input
            .lines()
            .filter(|line| *line == "-----BEGIN PGP PUBLIC KEY BLOCK-----")
            .count()
    );
    assert!(keys.len() > 10);
    assert!(matches!(
        verify_node(&vec![0; 1024 * 1024 + 1], SIGNATURE),
        Err(NodeSignatureError::InputLimit)
    ));
    assert!(matches!(
        verify_node(DATA, &vec![0; 64 * 1024 + 1]),
        Err(NodeSignatureError::InputLimit)
    ));
    assert!(verify_node(DATA, b"").is_err());
}
