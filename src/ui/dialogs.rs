use cursive::view::Nameable;
use cursive::views::{Dialog, EditView, LinearLayout, Panel, TextContent, TextView};
use cursive::{Cursive, CursiveRunner, XY};
use rand::distributions::{Alphanumeric, DistString};

/// Shows a small panel at the bottom right of the screen containing information. Useful for
/// letting the user know something is happening without interrupting their workflow. The panel
/// does not capture any actions, letting the UI continue to work without interruptions.
///
/// # Arguments
///
/// * `siv` - Cursive instance.
/// * `title` - Title for the panel
/// * `content` - Content inside the panel.
/// * `screen_size` - Size of the screen (needed for positioning the notification)
///

pub fn notification(
    siv: &mut CursiveRunner<Cursive>,
    title: &str,
    content: &str,
    screen_size: &XY<usize>,
) -> String {
    let uuid = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);

    let notif = Panel::new(TextView::new_with_content(TextContent::new(content)))
        .title(title)
        .with_name(uuid.clone());

    siv.add_layer(notif);

    // making the view non-modal prevents it from skipping over events!
    let front = cursive::views::LayerPosition::FromFront(0);
    siv.screen_mut().set_modal(front, false);

    // moves notification to bottom right corner of the screen
    siv.screen_mut().reposition_layer(
        front,
        cursive::view::Position::absolute((screen_size.x - content.len(), screen_size.y)),
    );
    uuid
}

/// Shortcut to write a dialog that asks for text input.
///
/// # Arguments
///
/// * `title` - Title for the dialog.
/// * `on_submit` - Function to run once submitted; uses contents of EditView as parameter.
/// * `secret` - whether or not the contents of the EditView should be rendered with stars
///
pub fn input_dialog<F: Fn(String) + std::marker::Sync + std::marker::Send + 'static>(
    title: &str,
    on_submit: F,
    secret: bool,
) -> Dialog {
    let mut ev = EditView::new().with_name("input");
    ev.get_mut().set_secret(secret);

    let mut dialog = Dialog::around(
        LinearLayout::new(cursive::direction::Orientation::Vertical)
            .child(TextView::new_with_content(TextContent::new(title)))
            .child(ev),
    );

    dialog.add_button("Submit", move |siv| {
        let new_name = siv
            .find_name::<EditView>("input")
            .expect("edit view disappeared")
            .get_content()
            .to_string();

        on_submit(new_name);
        siv.pop_layer();
    });

    dialog.add_button("Cancel", |siv| {
        siv.pop_layer();
    });

    dialog
}
