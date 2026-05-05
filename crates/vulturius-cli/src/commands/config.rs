use vulturius_core::config::AppConfig;

pub async fn handle(_config: &AppConfig) -> anyhow::Result<()> {
    println!("Config command — full implementation in Phase 1");
    Ok(())
}
