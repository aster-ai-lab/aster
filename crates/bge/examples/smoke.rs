//! One-shot local smoke test against the installed weights.
#[tokio::main]
async fn main() {
    let embed = aster_use_bge::EmbeddingClient::from_env().expect("embed client");
    println!("embedding status: {:?}", embed.status().available);
    let response = embed
        .embed(
            aster_use_bge::EmbeddingRequest::from_json(br#"{"input":["hello","aster os"]}"#)
                .expect("request"),
        )
        .await
        .expect("embed");
    println!(
        "embedding: {} items, {} dims, usage {:?}",
        response.data.len(),
        response.data[0].embedding.len(),
        response.usage
    );
    let rerank = aster_use_bge::RerankClient::from_env().expect("rerank client");
    println!("rerank status: {:?}", rerank.status().available);
    let response = rerank
        .rerank(
            aster_use_bge::RerankRequest::from_json(
                br#"{"query":"what is aster os","documents":["Aster OS is an operating system for agents","Bananas are yellow"],"top_n":2}"#,
            )
            .expect("request"),
        )
        .await
        .expect("rerank");
    for result in &response.results {
        println!(
            "rerank: index {} score {:.4}",
            result.index, result.relevance_score
        );
    }
}
