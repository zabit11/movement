use m1_da_light_node_runners::{
    Runner,
    celestia_appd::CelestiaAppd
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    
	let dot_movement = dot_movement::DotMovement::try_from_env()?;
    let path = dot_movement.get_path().join("config.toml");
    let config = m1_da_light_node_util::Config::try_from_toml_file(
        &path
    ).unwrap_or_default();

    let local = CelestiaAppd::local();
    local.run(
        dot_movement,
        config
    ).await?;

	Ok(())
}