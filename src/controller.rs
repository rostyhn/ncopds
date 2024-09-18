use crate::config::{write_to_config, Config};
use crate::connection::{Connection, LocalConnection, OnlineConnection};
use crate::model::EntryType;
use crate::server::{store_password, Server};
use crate::ui::uiroot::{UIMessage, UIRoot};
use crate::utils::{directory_str_to_url, rename_full_dir_fname};
use chrono::prelude::*;
use cursive::reexports::log::{log, Level};
use image::load_from_memory;
use keyring;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use opener::open;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{remove_dir, remove_file};
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use termsize;
use tokio::sync::Mutex;
use url::Url;

#[derive(Clone, Debug)]
pub enum ControllerMessage {
    /// runs when an entry is selected in the file view
    EntrySelected(EntryType),
    /// adds a connection  
    AddConnection(String, Server, Option<String>),
    /// changes the currently active connection
    ChangeConnection(String),
    /// moves up a directory in the current connection and updates the UI
    GoBack(),
    /// opens a file URL using the OS mimetype handler (e.g. xdg-open)
    Open(Url),
    /// moves the currently active connection to the specified URL
    Navigate(Url),
    /// downloads the file at the specified URL to the download directory
    Download(Url),
    /// downloads the image for the entry and stores it in the UI
    RequestImage(EntryType),
    /// renames a file
    Rename(PathBuf, PathBuf),
    /// deletes a file
    Delete(Url),
    /// uses the connection's available search function to search for a given string
    Search(String),
}

pub struct Controller {
    rx: mpsc::Receiver<ControllerMessage>,
    tx: mpsc::Sender<ControllerMessage>,
    pub ui: UIRoot,
    connections: HashMap<String, Arc<Mutex<dyn Connection>>>,
    current_tab: String,
    client: reqwest::Client,
    config: Config,
    config_path: Box<std::path::PathBuf>,
    refresh_timer: u32,
    download_directory: Url,
}

impl Controller {
    /// Builds the controller for the TUI. Sets up a connection to the directory specified in the
    /// config. The controller and UI communicate via mpsc channels but otherwise share no data in
    /// order to enforce separation of concerns.
    ///
    /// # Arguments
    ///
    /// * `config` - Config struct
    /// * `config_path` - Location of config on disk
    /// * `theme_path` - Location of theme file on disk
    /// * `t_size` - size of the terminal, used for rendering
    ///
    pub fn new(
        config: Config,
        config_path: &std::path::Path,
        theme_path: &std::path::Path,
        t_size: termsize::Size,
    ) -> Result<Controller, Box<dyn Error>> {
        let (tx, rx) = mpsc::channel::<ControllerMessage>();
        let download_directory = directory_str_to_url(&config.download_directory)?;

        let lc = LocalConnection::new(download_directory.clone());
        let client = reqwest::Client::builder()
            .user_agent("ncopds")
            .build()
            .unwrap();

        let ui = UIRoot::new(tx.clone(), theme_path, t_size);
        let mut connections = HashMap::<String, Arc<Mutex<dyn Connection>>>::new();

        connections.insert("local".to_string(), Arc::new(Mutex::new(lc)));

        Ok(Controller {
            rx,
            tx,
            ui,
            current_tab: "local".to_string(),
            connections,
            client,
            config,
            config_path: Box::new(config_path.to_owned()),
            download_directory,
            refresh_timer: 30 * 5 * 60, // fps * time in seconds
        })
    }

    /// Connects to servers specified in the config file. To do this, the function first iterates
    /// over each server in memory and retrieves its password from the OS keyring (if applicable).
    /// If the password is present (or unneeded), it establishes a connection and makes it
    /// available in the UI. Connections that are missing passwords ask the user to input the
    /// password, which is again stored in the OS keyring.
    ///
    /// # Panics
    ///
    /// Panics can occur if there is something wrong with the OS keyring.
    ///
    pub async fn connect_to_servers(&mut self) {
        // test
        let mut missing_passwords = vec![];
        let servers = self.config.servers.clone().unwrap_or_default();

        for (name, server) in servers.iter() {
            let mut missing_password = false;
            let password = match server.get_password() {
                Ok(pwd) => pwd,
                Err(err) => match err {
                    keyring::Error::NoEntry => {
                        missing_password = true;
                        None
                    }
                    err => {
                        panic!(
                            "Could not retrieve password for connection {:?}:{}",
                            server, err
                        );
                    }
                },
            };

            if !missing_password {
                self.tx
                    .send(ControllerMessage::AddConnection(
                        name.to_string(),
                        server.clone(),
                        password,
                    ))
                    .expect("could not send controller message");
            } else {
                missing_passwords.push(name);
            }
        }

        // not sure if maybe this should be moved out into a separate function
        for server_name in missing_passwords {
            let server = servers.get(server_name).unwrap();
            self.ui
                .ui_tx
                .send(UIMessage::PasswordPrompt(
                    server_name.clone(),
                    server.clone(),
                ))
                .expect("failed to send UI message");
        }
    }

    /// Sets the currently active connection, updating the UI.
    ///
    /// # Arguments
    ///
    /// * `id` - id of the connection
    ///
    pub async fn change_connection(&mut self, id: String) -> Result<(), Box<dyn Error>> {
        self.current_tab = id.clone();
        let connection = &self.connections[&id];
        self.navigate_to_async(connection, &connection.lock().await.current_address())
            .await?;
        Ok(())
    }

    /// Asynchronously moves the connection to the specified URL.
    ///
    /// # Arguments
    ///
    /// * `conn` - Connection to update.
    /// * `url` - URL to visit.
    ///
    pub async fn navigate_to_async(
        &self,
        conn: &Arc<Mutex<dyn Connection>>,
        url: &Url,
    ) -> Result<(), Box<dyn Error>> {
        let tx_clone = self.ui.ui_tx.clone();
        let c_clone = Arc::clone(conn);
        let p = url.clone();

        tokio::spawn(async move {
            let mut cloned = c_clone.lock().await;
            let e = cloned.navigate_to(&p).await;
            let addr = cloned.current_address().to_string();

            if let Ok(en) = e {
                tx_clone
                    .send(UIMessage::UpdateDirectoryView(addr, en, String::from("")))
                    .expect("failed to send UI message");
            } else {
                // perhaps should be more consistent as a msgbox
                tx_clone
                    .send(UIMessage::UpdateDirectoryView(
                        addr,
                        vec![],
                        format!("Load failed: {}", e.err().unwrap()).to_string(),
                    ))
                    .expect("failed to send UI message");
            }
        });

        self.ui.ui_tx.send(UIMessage::UpdateDirectoryView(
            url.to_string(),
            vec![],
            "Loading...".to_string(),
        ))?;

        Ok(())
    }

    /// Called when the user presses enter on a selection in the file view. Either opens a context
    /// menu for files or navigates into a directory.
    ///
    /// # Arguments
    ///
    /// * `item` - The item that was selected.
    ///
    fn entry_selected(&self, item: EntryType) -> Result<(), Box<dyn Error>> {
        match item {
            EntryType::File(title, url) => {
                let mut ctx_entries = vec![];
                ctx_entries.push(("Open".to_string(), ControllerMessage::Open(url.clone())));
                ctx_entries.push(("Delete".to_string(), ControllerMessage::Delete(url.clone())));

                let fp = url.to_file_path().expect("Somehow file path was wrong");
                ctx_entries.push((
                    String::from("Rename"),
                    ControllerMessage::Rename(fp.clone(), fp),
                ));

                self.ui
                    .ui_tx
                    .send(UIMessage::ShowContextMenu(title, ctx_entries))?;
                Ok(())
            }
            EntryType::Directory(title, url) => {
                self.tx.send(ControllerMessage::Navigate(url))?;
                Ok(())
            }
            EntryType::OPDSEntry(data) => {
                if let Some(rel) = data.unsupported {
                    let msg = format!("Unsupported acquisition type: {}", &rel);
                    return Err(msg.into());
                }

                // implies that this entry is a directory
                if let Some(href) = data.href {
                    self.tx.send(ControllerMessage::Navigate(href))?;
                    return Ok(());
                }

                if data.downloads.is_empty() {
                    return Err("Cannot perform any action on this entry.".into());
                }

                // build list of download entries
                let mut download_entries = vec![];
                for (href, mt) in data.downloads {
                    download_entries.push((
                        format!("Download as {}", mt).clone(),
                        ControllerMessage::Download(href),
                    ));
                }

                self.ui
                    .ui_tx
                    .send(UIMessage::ShowContextMenu(data.title, download_entries))?;

                Ok(())
            }
        }
    }

    /// Updates the configuration file with the data for the specified connection.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of server configuration to update.
    /// * `server` - Server data.
    ///
    fn update_config(&mut self, name: &str, server: &Server) -> Result<(), Box<dyn Error>> {
        self.config
            .servers
            .as_mut()
            .unwrap()
            .insert(name.to_string(), server.clone());

        write_to_config(&self.config, &self.config_path.to_owned())?;
        Ok(())
    }

    /// Function that reacts to messages from the UI.  
    ///
    /// # Arguments
    ///
    /// * `message` - Message from UI    
    ///
    async fn handle_messages(&mut self, message: ControllerMessage) -> Result<(), Box<dyn Error>> {
        let conn = self.connections.get(&self.current_tab).unwrap();
        let tx_clone = self.ui.ui_tx.clone();
        let c_clone = Arc::clone(conn);

        match message {
            ControllerMessage::EntrySelected(item) => {
                self.entry_selected(item)?;
                Ok(())
            }
            ControllerMessage::Open(p) => {
                open(p.to_file_path().unwrap())?;
                Ok(())
            }
            ControllerMessage::Delete(p) => {
                let path = p.to_file_path().unwrap();

                if path.is_dir() {
                    remove_dir(path)?;
                } else {
                    remove_file(path)?;
                }

                Ok(())
            }
            ControllerMessage::AddConnection(name, s, pwd) => {
                store_password(&s, &pwd);

                let oc = OnlineConnection::new(&s, self.client.clone(), pwd.clone()).await?;
                self.connections
                    .insert(name.clone(), Arc::new(Mutex::new(oc)));

                self.update_config(&name, &s)?;

                self.ui
                    .ui_tx
                    .send(UIMessage::AddConnection(name, s.clone(), pwd))?;

                Ok(())
            }
            ControllerMessage::ChangeConnection(url) => self.change_connection(url).await,
            ControllerMessage::GoBack() => {
                let mut mut_conn = conn.lock().await;
                let e = mut_conn.back().await?;
                self.ui.ui_tx.send(UIMessage::UpdateDirectoryView(
                    mut_conn.current_address().to_string(),
                    e,
                    String::from(""),
                ))?;
                Ok(())
            }
            ControllerMessage::Download(url) => {
                let download_directory = self.download_directory.clone();
                let url_name = url.to_string();

                tokio::spawn(async move {
                    let lock = c_clone.lock().await;
                    let oc: &OnlineConnection =
                        lock.as_any().downcast_ref::<OnlineConnection>().unwrap();
                    let res = oc.download(&url).await;

                    if res.is_ok() {
                        let (fname, data) = res.unwrap();
                        let res = crate::utils::save_as(data, &download_directory, &fname);

                        let msg = match res {
                            Ok(_) => format!("File {0} finished downloading", &fname),
                            Err(err) => err.to_string(),
                        };

                        tx_clone
                            .send(UIMessage::ShowNotification("Attention".to_string(), msg))
                            .expect("failed to send UI message");
                    } else {
                        tx_clone
                            .send(UIMessage::ShowInfo(
                                "Error".to_string(),
                                format!("Download from {} failed: {}", url, res.err().unwrap()),
                            ))
                            .expect("failed to send UI message");
                    }
                });

                self.ui.ui_tx.send(UIMessage::ShowNotification(
                    "Starting download".to_string(),
                    url_name,
                ))?;

                Ok(())
            }
            ControllerMessage::Navigate(p) => {
                self.navigate_to_async(conn, &p).await?;
                Ok(())
            }
            ControllerMessage::RequestImage(entry) => {
                match entry {
                    EntryType::File(title, url) => {
                        // TODO: implement rendering the first page of a pdf / epub
                        // load from disk
                    }
                    EntryType::Directory(title, url) => {
                        // return generic image
                    }
                    EntryType::OPDSEntry(data) => {
                        let title = data.title.clone();

                        if let Some(image_url) = data.image {
                            tokio::spawn(async move {
                                let lock = c_clone.lock().await;
                                let byte_data = lock.get_image_bytes(&image_url).await;
                                let id = load_from_memory(&byte_data).unwrap();
                                tx_clone
                                    .send(UIMessage::StoreImage(title.clone(), id))
                                    .expect("failed to send UI message");
                            });
                        }
                    }
                }
                Ok(())
            }
            ControllerMessage::Rename(old_path, new_path) => {
                rename_full_dir_fname(old_path, new_path)
            }
            ControllerMessage::Search(query) => {
                let mut mut_conn = conn.lock().await;
                let res = mut_conn.search(&query).await?;
                self.ui.ui_tx.send(UIMessage::UpdateDirectoryView(
                    format!("Search results for {}", query),
                    res,
                    String::from(""),
                ))?;

                Ok(())
            }
        }
    }

    /// Refreshes the currently active page. Called by the file watcher as well as by the main
    /// event loop on a timer.
    ///
    /// # Errors
    ///
    /// Errors related to querying the server.
    ///
    async fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
        let conn = self.connections.get(&self.current_tab).unwrap();
        let mut mut_conn = conn.lock().await;
        let cr = &mut_conn.current_address();
        let e = mut_conn.get_page(cr).await?;

        let msg = format!("Updated {}", Utc::now());

        self.ui.ui_tx.send(UIMessage::UpdateDirectoryView(
            mut_conn.current_address().to_string(),
            e,
            msg,
        ))?;
        Ok(())
    }

    /// Main loop that updates the controller's state as well as the UI's.
    ///
    /// # Errors
    ///
    /// All of the program's errors should be caught and displayed by the UI. Any errors that
    /// propagate up past this function to main will be related to message passing failing.
    ///
    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.change_connection("local".to_string()).await?;
        self.connect_to_servers().await;

        let mut frame = 0;
        let (wtx, wrx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(wtx, notify::Config::default())?;

        watcher
            .watch(
                self.config.download_directory.as_ref(),
                RecursiveMode::Recursive,
            )
            .expect("failed to watch directory");

        while self.ui.step(frame) {
            while let Some(message) = self.rx.try_iter().next() {
                let res = self.handle_messages(message).await;
                if res.is_err() {
                    self.ui.ui_tx.send(UIMessage::ShowInfo(
                        "Error".to_string(),
                        res.unwrap_err().to_string(),
                    ))?;
                }
            }

            while let Some(res) = wrx.try_iter().next() {
                if res.is_ok() && &self.current_tab == "local" {
                    self.refresh().await?;
                }
            }

            if frame % (30 * self.refresh_timer) == 0 && &self.current_tab != "local" {
                self.refresh().await?;
            }
            frame += 1;
        }
        Ok(())
    }
}
