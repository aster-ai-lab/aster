use aster_gateway::translation::ResponseTranslation;
use bytes::Bytes;
use futures_util::{stream, StreamExt};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn public_sse_adapter_preserves_utf8_and_refuses_an_unfinished_stream() {
    let wire = concat!(
        "data: {\"id\":\"c1\",\"created\":42,\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"你好\"},\"finish_reason\":null}]}\r\n\r\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\r\n\r\n",
        "data: [DONE]\r\n\r\n"
    );
    for direction in [
        ResponseTranslation::ToAnthropic,
        ResponseTranslation::ResponsesFromChat,
    ] {
        for split in 0..=wire.len() {
            let chunks = [&wire.as_bytes()[..split], &wire.as_bytes()[split..]]
                .into_iter()
                .map(|bytes| Ok::<_, io::Error>(Bytes::copy_from_slice(bytes)))
                .collect::<Vec<_>>();
            let mut translated = Box::pin(direction.translate_sse_stream(stream::iter(chunks), 42));
            let mut bytes = Vec::new();
            while let Some(chunk) = translated.next().await {
                bytes.extend(chunk.unwrap());
            }
            let text = String::from_utf8(bytes).unwrap();
            assert!(text.contains("你好"));
            let terminal = match direction {
                ResponseTranslation::ToAnthropic => "event: message_stop",
                ResponseTranslation::ResponsesFromChat => "event: response.completed",
                ResponseTranslation::ToOpenai => unreachable!(),
            };
            assert_eq!(text.matches(terminal).count(), 1);
        }
        let first = wire.split("\r\n\r\n").next().unwrap().to_owned() + "\r\n\r\n";
        let mut translated =
            Box::pin(direction.translate_sse_stream(stream::iter([Ok(Bytes::from(first))]), 42));
        let mut failed = false;
        while let Some(chunk) = translated.next().await {
            failed |= chunk.is_err();
        }
        assert!(failed);
    }
}

struct DropProbe(Arc<AtomicBool>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn public_sse_adapter_releases_upstream_on_terminal_and_caller_drop() {
    for terminal in [false, true] {
        let dropped = Arc::new(AtomicBool::new(false));
        let probe = DropProbe(Arc::clone(&dropped));
        let events = concat!(
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"m1\",\"model\":\"m\",\"usage\":{\"input_tokens\":1}}}\n\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n"
        ).to_owned() + if terminal { "data: {\"type\":\"message_stop\"}\n\n" } else { "" };
        let upstream = stream::unfold((Some(events), probe), |(events, probe)| async move {
            match events {
                Some(events) => Some((Ok(Bytes::from(events)), (None, probe))),
                None => std::future::pending().await,
            }
        });
        let mut translated =
            Box::pin(ResponseTranslation::ToOpenai.translate_sse_stream(upstream, 42));
        if terminal {
            tokio::time::timeout(std::time::Duration::from_secs(1), async {
                while let Some(chunk) = translated.next().await {
                    chunk.unwrap();
                }
            })
            .await
            .unwrap();
            assert!(dropped.load(Ordering::SeqCst));
        } else {
            translated.next().await.unwrap().unwrap();
            assert!(!dropped.load(Ordering::SeqCst));
        }
        drop(translated);
        assert!(dropped.load(Ordering::SeqCst));
    }
}
