use super::*;

#[test]
fn aggregate_event_bound_applies_across_short_data_lines() {
    let mut decoder = SseDecoder::default();
    let line = format!("data: {}\n", "x".repeat(4095));
    for _ in 0..MAX_EVENT_BYTES / 4096 {
        assert!(decoder.next_event(&mut line.as_bytes()).unwrap().is_none());
    }
    let error = decoder
        .next_event(&mut b"data: x\n".as_slice())
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn consumes_only_one_event_and_ignores_sse_control_fields() {
    let mut decoder = SseDecoder::default();
    let mut input =
        b": heartbeat\rid: a\revent: message\rdata: one\rdata:two\r\rdata: three\r\r".as_slice();
    assert_eq!(
        decoder.next_event(&mut input).unwrap().unwrap(),
        "one\ntwo\n"
    );
    assert_eq!(input, b"data: three\r\r");
    assert_eq!(decoder.next_event(&mut input).unwrap().unwrap(), "three\n");
    assert!(input.is_empty());
}
