use super::*;

#[test]
fn take_sse_events_accepts_crlf_tools_list() {
    let frame = "event: message\r\ndata: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"tools\":[{\"name\":\"lookup\"}]}}\r\n\r\n";
    let mut buffer = frame.to_string();
    let events = take_sse_events(&mut buffer, false);
    assert!(buffer.is_empty(), "the CRLF frame is one complete event");
    assert_eq!(events.len(), 1);
    let mut data = String::new();
    for line in events[0].lines() {
        if let Some(value) = data_field_value(line) {
            data.push_str(value);
        }
    }
    assert_eq!(
        data,
        "{\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"tools\":[{\"name\":\"lookup\"}]}}"
    );
    assert!(!data.contains('\r'));
}

#[test]
fn take_sse_events_keeps_lf_frames_and_a_split_crlf_pair() {
    let mut buffer = "data: {\"ok\":true}\n\n".to_string();
    assert_eq!(take_sse_events(&mut buffer, false).len(), 1);
    assert!(buffer.is_empty());

    let mut split = "data: {\"ok\":true}\r".to_string();
    assert!(
        take_sse_events(&mut split, false).is_empty(),
        "a trailing CR may be the start of CRLF"
    );
    assert!(split.ends_with('\r'));
    split.push_str("\n\r\n");
    let events = take_sse_events(&mut split, false);
    assert_eq!(events.len(), 1);
    assert!(split.is_empty());
    assert_eq!(
        events[0].lines().find_map(data_field_value),
        Some("{\"ok\":true}")
    );
}

#[test]
fn take_sse_events_skips_a_crlf_blank_line_without_stalling() {
    let mut buffer = "\r\n\r\ndata: {\"ok\":true}\r\n\r\n".to_string();
    let events = take_sse_events(&mut buffer, false);
    assert_eq!(events.len(), 1);
    assert!(buffer.is_empty());
}

#[test]
fn data_field_value_accepts_optional_space() {
    assert_eq!(
        data_field_value("data: {\"ok\":true}"),
        Some("{\"ok\":true}")
    );
    assert_eq!(
        data_field_value("data:{\"ok\":true}"),
        Some("{\"ok\":true}")
    );
    assert_eq!(data_field_value("data: [DONE]"), Some("[DONE]"));
    assert_eq!(data_field_value("data:[DONE]"), Some("[DONE]"));
}

#[test]
fn data_field_value_ignores_non_data_fields() {
    assert_eq!(data_field_value("event: message"), None);
    assert_eq!(data_field_value("id: 1"), None);
}

#[test]
fn utf8_stream_decoder_preserves_code_points_split_at_every_byte() {
    let input = "data: {\"title\":\"维护治理\"}\n\n";
    let mut decoder = Utf8StreamDecoder::default();
    let mut output = String::new();

    for byte in input.as_bytes() {
        decoder.push_to(&[*byte], &mut output).unwrap();
    }

    decoder.finish().unwrap();
    assert_eq!(output, input);
    assert!(!output.contains('\u{fffd}'));
}

#[test]
fn utf8_stream_decoder_rejects_invalid_bytes() {
    let mut decoder = Utf8StreamDecoder::default();
    assert!(decoder.push_to(&[0xff], &mut String::new()).is_err());
}

#[test]
fn incremental_scanning_preserves_fields_across_every_chunk_boundary() {
    let wire = "\r\n: keepalive\r\nid: 7\r\nevent: message\r\ndata: 中文\r\ndata: second\r\n\r\ndata: CR\r\rdata: LF\n\ndata: unterminated";
    let expected = [
        ": keepalive\r\nid: 7\r\nevent: message\r\ndata: 中文\r\ndata: second\r\n",
        "data: CR\r",
        "data: LF\n",
    ];
    for chunk_size in 1..=wire.len() {
        let mut buffer = String::new();
        let mut decoder = Utf8StreamDecoder::default();
        let mut scan = SseEventScan::default();
        let mut events = Vec::new();
        for chunk in wire.as_bytes().chunks(chunk_size) {
            decoder.push_to(chunk, &mut buffer).unwrap();
            events.extend(take_sse_events_scanned(&mut buffer, &mut scan, false));
        }
        decoder.finish().unwrap();
        events.extend(take_sse_events_scanned(&mut buffer, &mut scan, true));
        assert_eq!(events, expected, "chunk size {chunk_size}");
        assert_eq!(buffer, "data: unterminated");
    }
}

#[test]
fn coalesced_events_preserve_order_and_leave_only_the_incomplete_suffix() {
    let mut buffer = (0..5000)
        .map(|id| format!("id: {id}\r\ndata: {id}\r\n\r\n"))
        .collect::<String>();
    buffer.push_str("data: pending");
    let events = take_sse_events(&mut buffer, false);
    assert_eq!(events.len(), 5000);
    for (id, event) in events.iter().enumerate() {
        assert_eq!(event, &format!("id: {id}\r\ndata: {id}\r\n"));
    }
    assert_eq!(buffer, "data: pending");
}
