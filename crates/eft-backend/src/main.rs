mod api;
mod storage;
#[cfg(test)]
mod tests;

use std::{env, error::Error, net::SocketAddr, path::PathBuf};

use api::app;
use storage::{migrate_legacy_inventory, BackendState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let address: SocketAddr = address.parse()?;
    let inventory_file = if let Some(path) = env::var_os("INVENTORY_FILE") {
        PathBuf::from(path)
    } else {
        let local_app_data = env::var_os("LOCALAPPDATA")
            .ok_or("LOCALAPPDATA is not set; set INVENTORY_FILE to choose a profile path")?;
        let path = PathBuf::from(local_app_data)
            .join("Escape From Tarkov Rust")
            .join("inventory.json");
        migrate_legacy_inventory(&path, &PathBuf::from("data").join("inventory.json")).await?;
        path
    };
    let listener = TcpListener::bind(address).await?;

    println!("eft-backend listening on http://{address}");
    println!("inventory file: {}", inventory_file.display());
    axum::serve(listener, app(BackendState::new(inventory_file))).await?;
    Ok(())
}
