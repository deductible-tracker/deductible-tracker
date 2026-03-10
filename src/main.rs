#[tokio::main]
async fn main() -> anyhow::Result<()> {
	deductible_tracker::run_app().await
}
