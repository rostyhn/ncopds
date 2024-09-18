// perhaps rename to Entry?

use atom_syndication::Entry;
use url::Url;

#[derive(Debug, Clone)]
pub struct EntryData {
    pub title: String,
    pub details: String,
    pub author: Option<String>,
    pub unsupported: Option<String>,
    pub downloads: Vec<(Url, String)>,
    pub image: Option<Url>,
    pub href: Option<Url>,
}

#[derive(Debug, Clone)]
pub enum EntryType {
    File(String, Url),
    Directory(String, Url),
    OPDSEntry(EntryData),
}

// add test
/// Converts an atom_syndication::Entry into a ncopds::EntryType. These are represented in the UI
/// as entries in the file view (left side of the screen).
///
/// # Arguments
///
/// * `entry` - Entry to convert.
/// * `base_url` - Domain of OPDS this entry was retrieved from.
///
/// # Errors
///
/// Errors related to parsing can occur.
///
pub fn process_opds_entry(
    entry: &Entry,
    base_url: &Url,
) -> Result<EntryType, Box<url::ParseError>> {
    let authors = entry.authors();
    let summary = entry.summary();
    let content = entry.content();
    let categories = entry.categories();

    let mut entry_details = String::from("");
    let mut author = None;

    if !authors.is_empty() {
        author = Some(
            authors
                .iter()
                .map(|x| x.name.clone())
                .collect::<Vec<String>>()
                .join(","),
        );
    }

    if let Some(s) = summary {
        entry_details += &format!("Summary: {0}\n\n", s.to_string());
    }

    if let Some(c) = content {
        entry_details += &format!("{}\n", c.value().unwrap());
    }

    if !categories.is_empty() {
        let cat_string = categories
            .iter()
            .map(|x| x.label().unwrap_or(""))
            .collect::<Vec<&str>>()
            .join(",");
        entry_details += &format!("Categories: {0}", cat_string);
    }

    let mut downloads = vec![];
    let mut image = None;

    let mut f_href = None;
    let mut unsupported = None;

    for link in entry.links() {
        let href = crate::utils::parse_href(&link.href, base_url)?;
        let rel = link.rel();

        // unsupported acquisition types for now
        if rel.contains("acquisition")
            && (rel.contains("borrow")
                || rel.contains("buy")
                || rel.contains("subscribe")
                || rel.contains("sample"))
        {
            unsupported = Some(String::from(rel));
        }

        let mt = link
            .mime_type()
            .expect("malformed feed, expected mime-type");

        // this makes it into a directory
        if mt.contains("application/atom+xml") {
            f_href = Some(href);
        } else if mt.contains("image") {
            image = Some(href);
        } else {
            downloads.push((href, String::from(mt)));
        }
    }

    Ok(EntryType::OPDSEntry(EntryData {
        title: entry.title().to_string(),
        author,
        details: entry_details,
        unsupported,
        downloads,
        image,
        href: f_href,
    }))
}

/// Convenience method to retrieve the title for an Entry
///
/// # Arguments
///
/// * `e` - The entry to retrieve the title for.
///
pub fn get_title_for_entry(e: &EntryType) -> String {
    match e {
        EntryType::File(t, _) => t.to_string(),
        EntryType::Directory(t, _) => t.to_string(),
        EntryType::OPDSEntry(data) => data.title.clone(),
    }
}
