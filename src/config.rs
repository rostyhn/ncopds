use crate::server::Server;
use crate::Error;
use cursive::reexports::log::{log, Level};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{create_dir_all, read_to_string, File};
use std::io::{ErrorKind, Write};
use std::path::Path;
use toml;

// this is joined with $HOME when the program first launches
pub const CONFIG_DIRECTORY: &str = "/.config/ncopds/";

#[derive(Deserialize, Debug, Serialize)]
pub struct Config {
    pub download_directory: String,
    pub servers: Option<HashMap<String, Server>>,
}

/// Creates a default config at the path specified. All it contains is a line for the download
/// directory to be set at $HOME.
///
/// # Arguments
///
/// * `file_path` - The path to save the config to.
///
pub fn create_default_config(file_path: &Path) -> Result<File, std::io::Error> {
    // test
    create_dir_all(file_path.parent().unwrap())?;

    let mut fc = match File::create(file_path) {
        Ok(f) => f,
        Err(e) => return Err(e),
    };

    let home = env::var("HOME").unwrap().to_string();

    // minimal config needed for the program to work
    let default_config = format!("download_directory = '{}'", &home);

    fc.write_all(default_config.as_bytes())
        .expect("Unable to write data");

    Ok(fc)
}

/// Writes config to file path.
pub fn write_to_config(config: &Config, file_path: &Path) -> Result<(), Box<dyn Error>> {
    // add test, rename?
    let s = toml::ser::to_string(config)?;
    let mut file = File::options().write(true).open(file_path)?;
    file.write_all(s.as_bytes())?;
    Ok(())
}

/// Read config from file path. If no config exists at the path specified, a default one is
/// created.
pub fn read_config(file_path: &Path) -> Result<Config, Box<dyn Error>> {
    // add test
    let contents = match read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => match e.kind() {
            ErrorKind::NotFound => match create_default_config(file_path) {
                Ok(_fc) => {
                    println!("Creating config file at {:?}", file_path);
                    read_to_string(file_path).unwrap()
                }
                Err(e) => panic!("Problem creating the configuration file: {:?}", e),
            },
            oe => {
                panic!("Problem opening the configuration file: {:?}", oe);
            }
        },
    };

    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}
