use crate::model::{get_title_for_entry, process_opds_entry, EntryType};
use crate::server::Server;
use crate::utils::{parse_href, read_dir};

use async_trait::async_trait;
use atom_syndication::Feed;
use bytes::Bytes;
use cursive::reexports::log::{log, Level};
use roxmltree::Document;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec::Vec;
use url::Url;

#[async_trait]
pub trait Connection: Send {
    /// Returns the content of the URL as a vector of entries
    async fn get_page(&mut self, addr: &Url) -> Result<Vec<EntryType>, Box<dyn Error>>;
    /// the currently active URL for the connection
    fn current_address(&self) -> Url;
    /// calls get_page and updates the history stack
    async fn navigate_to(&mut self, s: &Url) -> Result<Vec<EntryType>, Box<dyn Error>>;
    /// pops a page off of the history stack and returns the contents of the previous page
    async fn back(&mut self) -> Result<Vec<EntryType>, Box<dyn Error>>;
    /// gets data from the image at the URL
    async fn get_image_bytes(&self, addr: &Url) -> Bytes;
    /// uses the connection's search capabilities to run a search
    async fn search(&mut self, query: &str) -> Result<Vec<EntryType>, Box<dyn Error>>;
    fn as_any(&self) -> &dyn Any;
}

/// represents a connection to the local disk
pub struct LocalConnection {
    history: Vec<Url>,
    pub init_dir: Url,
}

impl LocalConnection {
    pub fn new(init_dir: Url) -> LocalConnection {
        LocalConnection {
            history: vec![],
            init_dir,
        }
    }
}

#[async_trait]
impl Connection for LocalConnection {
    fn current_address(&self) -> Url {
        // test
        self.history.last().unwrap_or(&self.init_dir).clone()
    }

    async fn get_page(&mut self, addr: &Url) -> Result<Vec<EntryType>, Box<dyn Error>> {
        // add test
        let fnames = read_dir(addr)?;

        Ok(fnames
            .iter()
            .map(|fname| {
                let full_path = Url::parse(&format!("{0}/{1}", addr, fname)).unwrap();
                let md = fs::metadata(full_path.to_file_path().unwrap()).unwrap();

                if md.is_file() {
                    EntryType::File(fname.to_string(), full_path)
                } else {
                    EntryType::Directory(fname.to_string(), full_path)
                }
            })
            .collect())
    }

    async fn navigate_to(&mut self, addr: &Url) -> Result<Vec<EntryType>, Box<dyn Error>> {
        // push history on regardless, user will pop it on failure
        self.history.push(addr.clone());
        self.get_page(addr).await
    }

    async fn back(&mut self) -> Result<Vec<EntryType>, Box<dyn Error>> {
        // add test
        if !self.history.is_empty() {
            self.history.pop();
            return self.get_page(&self.current_address()).await;
        }
        Err("At directory root; cannot go back.".into())
    }

    async fn get_image_bytes(&self, addr: &Url) -> Bytes {
        // TODO: implement image rendering for local files
        // should be reading byte info from file
        Bytes::new()
    }

    async fn search(&mut self, query: &str) -> Result<Vec<EntryType>, Box<dyn Error>> {
        // basically just filter on the results of navigate to
        // we are deliberately adding onto the history so it's easy to use back()
        let current_directory = self.navigate_to(&self.current_address()).await;
        Ok(current_directory
            .unwrap()
            .into_iter()
            .filter(|x| get_title_for_entry(x).contains(query))
            .collect())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Debug)]
pub struct OnlineConnection {
    /// server contains base_url and username
    pub server_info: Server,
    history: Vec<Url>,
    client: reqwest::Client,
    cache: HashMap<Url, Vec<EntryType>>,
    /// password for authentication, read from keyring
    password: Option<String>,
    /// URL used to build search queries
    search_url: Option<String>,
}

/// Helper function to build a request with authentication
///
/// # Arguments
///
/// * `client` - reqwest client
/// * `url` - url to request
/// * `username` - username for authentication
/// * `password` - password for authentication
///
fn build_req(
    client: &reqwest::Client,
    url: &Url,
    username: &Option<String>,
    password: &Option<String>,
) -> reqwest::RequestBuilder {
    let req = client.get(url.to_string());

    if let Some(u) = username {
        return req.basic_auth(u, password.clone());
    };

    req
}

/// Parses an opensearchdescription document to get the search url hidden within it. Returns none
/// if the document did not have a <Url> tag pointing to an Atom feed.
///
/// # Arguments
///
/// * `osd` - pointer to xml document struct
///
fn parse_osd(osd: &Document) -> Option<String> {
    let search_el = osd.descendants().find(|x| {
        x.tag_name().name() == "Url"
            && x.attribute("type")
                .is_some_and(|t| t.contains("application/atom+xml"))
    });

    if let Some(el) = search_el {
        el.attribute("template").map(|t| t.to_string())
    } else {
        None
    }
}

/// Attempts to find the URL used for searching an OPDS catalog. According to the [OPDS
/// spec](https://specs.opds.io/), the feed should have a link called "search" that points to
/// another XML document that has the relevant information.
///
/// # Arguments
///
/// * `client` - reqwest client
/// * `doc` - atom feed struct
/// * `s` - server information  
/// * `password` - password
///
async fn find_search_url(
    client: &reqwest::Client,
    doc: Feed,
    s: &Server,
    password: &Option<String>,
) -> Option<String> {
    let mut search_url = None;
    for l in doc.links {
        if let Some(mt) = l.mime_type() {
            if l.rel == "search" && mt.contains("opensearchdescription") {
                let u = parse_href(l.href(), &s.get_domain()).expect("");

                let osd_res = build_req(client, &u, &s.username, password)
                    .send()
                    .await
                    .ok()?;

                let b = &osd_res.bytes().await.ok()?;

                let bs = std::str::from_utf8(b).ok()?;
                let osd = Document::parse(bs).ok()?;
                let search_str = parse_osd(&osd)?;
                search_url = Some(parse_href(&search_str, &s.get_domain()).ok()?.to_string());
            }
        }
    }
    search_url
}

impl OnlineConnection {
    pub async fn new(
        s: &Server,
        client: reqwest::Client,
        password: Option<String>,
    ) -> Result<OnlineConnection, Box<dyn Error>> {
        // test connection
        let req = build_req(&client, &s.base_url, &s.username, &password);
        let response = req.send().await?;
        response.error_for_status_ref()?;

        let response_bytes = &response.bytes().await?;
        let doc = Feed::read_from(response_bytes.as_ref())?;
        let search_url = find_search_url(&client, doc, s, &password).await;

        let oc = OnlineConnection {
            history: vec![],
            server_info: s.clone(),
            client,
            cache: HashMap::new(),
            password,
            search_url,
        };

        Ok(oc)
    }

    /// Shorthand for build_req; builds a request for the URL using the credentials for the
    /// connection.
    ///
    /// # Arguments
    ///
    /// * `url` - URL to build request for
    ///
    pub fn get_request(&self, url: &Url) -> reqwest::RequestBuilder {
        build_req(
            &self.client,
            url,
            &self.server_info.username,
            &self.password,
        )
    }

    /// Returns the filename and byte data from the URL specified.
    ///
    /// # Arguments
    ///
    /// * `url` - URL to download from
    ///
    /// # Errors
    ///
    /// Errors related to making GET requests can arise.
    ///
    pub async fn download(&self, url: &Url) -> Result<(String, Bytes), Box<dyn Error>> {
        // add test
        let response = self.get_request(url).send().await?;
        let headers = &response.headers().to_owned();
        let response_bytes = response.bytes().await?;

        // basically all we do here is try and build up a filename
        let cd = headers.get("content-disposition");
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();

        let filename = url.path_segments().unwrap().last().unwrap_or(&t);

        if let Some(content_dispo) = cd {
            let cd_filename =
                crate::utils::extract_filename_from_content_disposition(content_dispo);

            if let Some(fname) = cd_filename {
                return Ok((fname.to_string(), response_bytes));
            }
        }

        Ok((filename.to_string(), response_bytes))
    }
}

#[async_trait]
impl Connection for OnlineConnection {
    async fn get_page(&mut self, addr: &Url) -> Result<Vec<EntryType>, Box<dyn Error>> {
        if let Some(d) = self.cache.get(addr) {
            return Ok(d.to_vec());
        };

        let response = self.get_request(addr).send().await?;
        response.error_for_status_ref()?;

        let response_bytes = response.bytes().await?;
        let doc = Feed::read_from(response_bytes.as_ref())?;

        // try and fix errors on feed if possible
        // https://github.com/rust-syndication/atom/blob/master/src/feed.rs
        // should be able to call Feed::from_xml on feeds that fail invalid start tags

        let mut entries = vec![];

        for entry in doc.entries().iter() {
            let processed_entry = process_opds_entry(entry, &self.server_info.get_domain())?;
            entries.push(processed_entry);
        }

        self.cache.insert(addr.clone(), entries.clone());
        Ok(entries)
    }

    async fn navigate_to(&mut self, addr: &Url) -> Result<Vec<EntryType>, Box<dyn Error>> {
        self.history.push(addr.clone());
        self.get_page(addr).await
    }

    // add test
    async fn back(&mut self) -> Result<Vec<EntryType>, Box<dyn Error>> {
        if !self.history.is_empty() {
            self.history.pop();
            return self.get_page(&self.current_address()).await;
        }
        Err("At ODPS root; cannot go back.".into())
    }

    fn current_address(&self) -> Url {
        match self.history.last() {
            Some(h) => h.clone(),
            None => self.server_info.base_url.clone(),
        }
    }

    async fn get_image_bytes(&self, addr: &Url) -> Bytes {
        let response = self.get_request(addr).send().await;

        match response {
            Ok(r) => r.bytes().await.unwrap_or(Bytes::new()),
            Err(_) => Bytes::new(),
        }
    }

    async fn search(&mut self, query: &str) -> Result<Vec<EntryType>, Box<dyn Error>> {
        // move to fn, add tests
        // https://specs.opds.io/opds-1.2#3-search
        // need to add support for advanced search fields
        if let Some(su) = &self.search_url {
            let target = su.replace("{searchTerms}", query);
            let tu = Url::parse(&target)?;
            self.navigate_to(&tu).await
        } else {
            Err("Server does not have searching enabled.".into())
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::{read_config, Config, CONFIG_DIRECTORY};
    use crate::env;
    use crate::Path;

    // TODO: add test config
    #[test]
    fn test_connection() {
        // NOTE: the test assumes that you have a valid config set up
        /*
        let home = env::var("HOME").unwrap().to_string();
        // read config file & check that all values are declared
        let config: Config = read_config(Path::new(&(home + CONFIG_DIRECTORY)));

        let s_info = config.servers.unwrap();
        let server = s_info.values().next().unwrap();

        let conn = OnlineConnection::new(&server).unwrap();
        let f = conn.get_page();

        print!("{:?}", f);*/
    }
}
