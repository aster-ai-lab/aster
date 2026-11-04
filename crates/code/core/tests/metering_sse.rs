use aster_core::llm::metering::wire::{SseMeter, UsageProtocol, MAX_SSE_FRAME_BYTES};
use serde_json::json;

const FRAME_LIMIT: usize = MAX_SSE_FRAME_BYTES;

#[test]
fn accepts_eight_mib_payloads_with_split_utf8_and_all_sse_line_endings() {
    let text = "中".repeat((8 * 1024 * 1024 - 512) / 3);
    let event = json!({"model":"fixture","choices":[{"delta":{"content":text}}]}).to_string();
    for newline in ["\n", "\r\n", "\r"] {
        let wire = format!("event: message{newline}data: {event}{newline}{newline}data: {{\"choices\":[],\"usage\":{{\"prompt_tokens\":7,\"completion_tokens\":3}}}}{newline}{newline}data: [DONE]{newline}{newline}");
        let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
        let mut frames = Vec::new();
        for chunk in wire.as_bytes().chunks(4093) {
            frames.extend(meter.push(chunk).unwrap());
        }
        frames.extend(meter.finish().unwrap());
        assert_eq!(frames.len(), 3);
        assert!(frames[0].has_output);
        assert!(frames[0].frame.contains(&event));
        assert!(!frames[0].frame.contains('\u{fffd}'));
        assert!(meter.meter.finished);
        assert!(meter.meter.usage_complete);
        assert_eq!(
            meter.meter.usage().unwrap().total_tokens().unwrap(),
            Some(10)
        );
        assert_eq!(meter.meter.usage().unwrap().cache_read_tokens, None);
    }
}

#[test]
fn bounds_individual_frames_instead_of_coalesced_transport_bytes() {
    let event = json!({"choices":[{"delta":{"content":"x".repeat(32 * 1024)}}]}).to_string();
    let wire = format!("data: {event}\n\n").repeat(600) + "data: [DONE]\n\n";
    assert!(wire.len() > FRAME_LIMIT);
    let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
    let frames = meter.push(wire.as_bytes()).unwrap();
    assert_eq!(frames.len(), 601);
    assert!(frames[..600].iter().all(|frame| frame.has_output));
    assert!(frames[0].frame.contains(&event));
    assert!(frames[599].frame.contains(&event));
    assert!(meter.meter.finished);
    assert!(!meter.meter.usage_complete);
}

#[test]
fn rejects_an_oversized_incomplete_frame_for_whole_and_fragmented_input() {
    for chunk_size in [4093, FRAME_LIMIT + 1] {
        let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
        meter
            .push(b"data: {\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3}}\n\n")
            .unwrap();
        assert!(meter.meter.usage_complete);
        let bytes = vec![b'x'; FRAME_LIMIT + 1];
        let mut rejected = false;
        for chunk in bytes.chunks(chunk_size) {
            if meter.push(chunk).is_err() {
                rejected = true;
                break;
            }
        }
        assert!(rejected);
        assert!(meter.meter.failed);
        assert!(!meter.meter.usage_complete);
        assert!(meter.buffered_bytes() <= FRAME_LIMIT);
        assert_eq!(
            meter.meter.usage().unwrap().total_tokens().unwrap(),
            Some(10)
        );
    }
}

#[test]
fn utf8_carry_counts_toward_the_limit_and_debug_keeps_content_private() {
    let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
    let private = "synthetic-secret-message";
    meter.push(private.as_bytes()).unwrap();
    meter.push(&[0xe4, 0xb8]).unwrap();
    assert_eq!(meter.buffered_bytes(), private.len() + 2);
    assert!(!format!("{meter:?}").contains(private));
    assert!(meter.finish().is_err());
    assert!(meter.meter.failed);

    let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
    meter.push(&vec![b'x'; FRAME_LIMIT - 2]).unwrap();
    meter.push(&[0xe4, 0xb8]).unwrap();
    assert_eq!(meter.buffered_bytes(), FRAME_LIMIT);
    assert!(meter.push(&[0xad]).is_err());
    assert_eq!(meter.buffered_bytes(), FRAME_LIMIT);
    assert!(meter.meter.failed);
}

#[test]
fn the_raw_buffer_accepts_boundary_frames_and_refuses_oversized_content() {
    for newline in ["\n", "\r\n", "\r"] {
        let suffix = format!("{newline}data: [DONE]{newline}{newline}");
        let wire = format!(":{}{suffix}", "x".repeat(FRAME_LIMIT - suffix.len() - 1));
        assert_eq!(wire.len(), FRAME_LIMIT);
        let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
        let mut frames = meter.push(wire.as_bytes()).unwrap();
        frames.extend(meter.finish().unwrap());
        assert_eq!(frames.len(), 1);
        assert_eq!(meter.buffered_bytes(), 0);
        assert!(meter.meter.finished);

        let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
        let frames = meter
            .push(format!("{wire}: keepalive\n\n").as_bytes())
            .unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(meter.buffered_bytes(), 0);
        assert!(meter.meter.finished);

        if newline == "\r\n" {
            let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
            // The blank CR commits before the following LF, which needs no carry.
            let frames = meter.push(format!(":{wire}").as_bytes()).unwrap();
            assert_eq!(frames.len(), 1);
            assert_eq!(meter.buffered_bytes(), 0);
        }

        let wire = format!("::{wire}");
        let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
        assert!(meter.push(wire.as_bytes()).is_err());
        assert!(meter.meter.failed);
        assert!(meter.buffered_bytes() <= FRAME_LIMIT);
    }
}

#[test]
fn a_cr_terminal_is_available_before_another_byte_or_eof() {
    let mut meter = SseMeter::new(UsageProtocol::OpenaiChat);
    let frames = meter.push(b"data: [DONE]\r\r").unwrap();
    assert_eq!(frames.len(), 1);
    assert!(meter.meter.finished);
    assert_eq!(meter.buffered_bytes(), 0);
    assert!(meter.push(b"\n").unwrap().is_empty());
    assert!(meter.finish().unwrap().is_empty());
}
