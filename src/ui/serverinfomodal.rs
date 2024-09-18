use cursive::traits::Nameable;
use cursive::views::{Dialog, EditView, LinearLayout, TextContent, TextView};
use cursive::Cursive;
use std::sync::mpsc;

use crate::server::Server;
use crate::ControllerMessage;
use url::Url;

/// Creates a dialog used for adding / editing a connection to the server. Serves as an alternative
/// to manually editing the config file.
///
/// # Arguments
///
/// * `sender` - Controller message channel.
///
pub fn new(sender: mpsc::Sender<ControllerMessage>) -> Dialog {
    Dialog::new()
        .title("Enter server information")
        .content(
            LinearLayout::vertical()
                .child(TextView::new_with_content(TextContent::new(
                    "Connection Name",
                )))
                .child(EditView::new().with_name("name"))
                .child(TextView::new_with_content(TextContent::new("Server URL")))
                .child(EditView::new().with_name("url"))
                .child(TextView::new_with_content(TextContent::new("Username")))
                .child(EditView::new().with_name("username"))
                .child(TextView::new_with_content(TextContent::new("Password")))
                .child(EditView::new().secret().with_name("password")),
        )
        .button("Ok", move |s| {
            let name = s.find_name::<EditView>("name").unwrap().get_content();
            let url = s.find_name::<EditView>("url").unwrap().get_content();

            let username = s
                .find_name::<EditView>("username")
                .unwrap()
                .get_content()
                .to_string();

            let password = s
                .find_name::<EditView>("password")
                .unwrap()
                .get_content()
                .to_string();

            // move to fn, test
            if !name.is_empty() && !url.is_empty() {
                let res = Url::parse(&url);
                match res {
                    Ok(parsed_url) => {
                        sender
                            .send(ControllerMessage::AddConnection(
                                name.to_string(),
                                Server {
                                    base_url: parsed_url,
                                    username: (!username.is_empty()).then_some(username),
                                },
                                (!password.is_empty()).then_some(password),
                            ))
                            .expect("failed to send UI message");
                    }
                    Err(err) => {
                        Dialog::info(err.to_string());
                    }
                }

                close(s);
            } else {
                Dialog::info("Name and URL fields cannot be empty!");
            }
        })
        .button("Cancel", close)
}

/// Meant to be called after a ServerInfoModal is created. Populates the fields of the modal with
/// information to make editing existing connections easier.
///
/// # Arguments
///
/// * `s` - Cursive instance
/// * `name` - Name of the connection.
/// * `server` - Server data struct
/// * `pwd` - Password for authentication.
///
pub fn populate_fields(s: &mut Cursive, name: &str, server: &Server, pwd: Option<String>) {
    s.find_name::<EditView>("name")
        .unwrap()
        .set_content(name.to_string());
    s.find_name::<EditView>("url")
        .unwrap()
        .set_content(server.base_url.to_string());

    match &server.username {
        Some(u) => {
            s.find_name::<EditView>("username")
                .unwrap()
                .set_content(u.to_string());
        }
        None => {}
    }

    match &pwd {
        Some(p) => {
            s.find_name::<EditView>("password").unwrap().set_content(p);
        }
        None => {}
    }
}

/// shortcut for closing the dialog
fn close(s: &mut Cursive) {
    s.pop_layer();
}
