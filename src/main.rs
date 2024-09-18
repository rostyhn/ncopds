pub mod config;
pub mod connection;
pub mod controller;
pub mod model;
pub mod server;
pub mod ui;
pub mod utils;

use config::{read_config, Config, CONFIG_DIRECTORY};
use controller::{Controller, ControllerMessage};
use std::env;
use std::error::Error;
use std::path::Path;
extern crate termsize;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // TODO: move into separate function, work towards supporting mac & win
    if std::env::consts::OS != "linux" {
        println!("Warning: your operating system is not currently supported. You may run into strange bugs and features not working correctly! Press any key to continue.");
        let mut s = String::new();
        let _ = std::io::stdin().read_line(&mut s);
    }

    let home = env::var("HOME").expect("could not read $HOME").to_string();
    let t_size = termsize::get().expect("could not read terminal size");

    let tp = format!("{}{}{}", home, CONFIG_DIRECTORY, "theme.toml");
    let cp = format!("{}{}{}", home, CONFIG_DIRECTORY, "config.toml");
    let config: Config = read_config(Path::new(&cp)).expect("Invalid config");

    let controller = Controller::new(config, Path::new(&cp), Path::new(&tp), t_size);
    match controller {
        Ok(mut controller) => controller.run().await?,
        Err(e) => println!("Fatal error: {}", e),
    };
    Ok(())
}
