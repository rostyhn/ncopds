use infer;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use url::Url;

/// Returns the contents of a directory.
///
/// # Arguments
///
/// * `file_path` - Url pointing to a directory.
///
/// # Errors
///
/// Errors related to reading the filesystem
///
pub fn read_dir(file_path: &Url) -> Result<Vec<String>, Box<dyn Error>> {
    let paths = fs::read_dir(file_path.to_file_path().unwrap())?;

    Ok(paths
        .map(|p| p.unwrap().file_name().into_string().unwrap())
        .collect())
}

/// Saves bytes in a file specified by dir and fname. Checks magic bytes using
/// [infer](https://docs.rs/infer/latest/infer/) and errors out if extension doesn't match the
/// filetype given by the magic bytes.
///
/// # Arguments
///
/// * `data` - Bytes containing file data
/// * `dir` - Directory to save the file in
/// * `fname` - Filename
///
/// # Errors
///
/// Can error out on file creation, joining directory with filename or when the file extension of
/// the filename does not match the magic bytes in the file.
///
/// ```
pub fn save_as(data: bytes::Bytes, dir: &Url, fname: &str) -> Result<(), Box<dyn Error>> {
    let full_fname = Url::join(dir, fname).unwrap().to_file_path().unwrap();

    // move extension testing into fn, test
    let ext = full_fname.extension();
    let kind = infer::get(&data).expect("file type is known");

    if kind.extension() != ext.unwrap() {
        return Err(format!(
            "Could not save {}. File was not downloaded properly. File was returned from the server as a {}",
            fname,
            kind.extension()
        )
        .into());
    }

    let mut file = File::create(&full_fname)?;
    let _ = file.write(&data);
    Ok(())
}

/// Converts a string file path to a URL.
///
/// # Arguments
///
/// * `s` - string to convert to URL
///
pub fn str_to_file_url(s: &str) -> Result<Url, url::ParseError> {
    Url::parse(&format!("file://{}", s))
}

/// Checks if a URL points to an existing directory.
///
/// # Arguments
///
/// * `u` - URL to test.
///
pub fn file_url_is_dir(u: &Url) -> bool {
    let p = u.to_file_path();

    if let Ok(fp) = p {
        fp.try_exists().expect("unable to check existence of file") && fp.is_dir()
    } else {
        false
    }
}

/// Converts a string expected to be a file path to a directory to a URL.
///
/// # Arguments
///
/// * `directory` - the string to convert.
///
/// # Errors
///
/// throws errors if
/// - the directory does not exist
/// - the string cannot be parsed
/// - the filepath points to something other than a directory
///
pub fn directory_str_to_url(directory: &str) -> Result<Url, Box<dyn Error>> {
    let init_dir = str_to_file_url(directory)?;

    if !file_url_is_dir(&init_dir) {
        return Err(format!("{} is not a directory.", directory).into());
    }

    Ok(init_dir)
}

/// Renames a file at old_path with the name in new_path. new_path is just the filename, the
/// function uses the parent directory of old_path to correctly rename the file.
///
/// # Arguments
///
/// * `old_path` - Path to old file.
/// * `new_path` - Filename of new file
///
/// # Errors
/// Error could get thrown if the operation fails.
///
///
pub fn rename_full_dir_fname(old_path: PathBuf, new_path: PathBuf) -> Result<(), Box<dyn Error>> {
    // is this necessary though?
    let folder = old_path.parent().expect("we should be inside a folder");
    let np = folder.join(&new_path);
    std::fs::rename(old_path, np)?;
    Ok(())
}

/// Parse a string into a URL. If the string is missing the domain, joins the string with base_url
/// to get an absolute URL.
///
/// # Arguments
///
/// * `href` - string to convert
/// * `base_url` - URL to join with href
///
/// # Errors
///
/// Will throw parsing errors that are not related to missing a base url.
///
pub fn parse_href(href: &str, base_url: &Url) -> Result<Url, url::ParseError> {
    Ok(match Url::parse(href) {
        Ok(res) => res,
        Err(e) => match e {
            url::ParseError::RelativeUrlWithoutBase => Url::join(base_url, href)?,
            _ => return Err(e),
        },
    })
}

/// Attempts to extract a filename from content-disposition headers.
///
/// # Arguments
///
/// * `cd` - [Content-disposition headers](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition)
///
pub fn extract_filename_from_content_disposition(
    cd: &reqwest::header::HeaderValue,
) -> Option<String> {
    let cd_str = cd.to_str().ok()?;

    let split: Vec<&str> = cd_str
        .split(";")
        .filter(|x| x.starts_with(" filename="))
        .collect();

    if split.is_empty() {
        return None;
    }

    Some(
        split
            .first()
            .unwrap()
            .strip_prefix(" filename=")
            .unwrap()
            .replace("%20", " "),
    )
}
