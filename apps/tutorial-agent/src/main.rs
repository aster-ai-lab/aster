use tutorial_agent::{
    runtime::{Arguments, Runtime},
    ui,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Arguments::parse(std::env::args_os().skip(1))?;
    let mut runtime = Runtime::open(&args).await?;
    let result = if let Some(prompt) = args.prompt {
        tutorial_agent::runtime::inspect_stream(&runtime.session, &prompt).await
    } else {
        let mut terminal = ratatui::init();
        let result = ui::run(&mut runtime, &mut terminal).await;
        ratatui::restore();
        result
    };
    let closed = runtime.shutdown().await;
    closed?;
    result
}
