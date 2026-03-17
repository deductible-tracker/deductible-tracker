fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    deductible_tracker::prepare_runtime_assets()
}