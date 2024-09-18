use crate::model::EntryType;
use crate::ui::canvas::CanvasView;

use cursive::view::Nameable;
use cursive::views::{
    DummyView, HideableView, LinearLayout, PaddedView, Panel, ResizedView, ScrollView, TextView,
};
use cursive::Cursive;
use cursive::Vec2;
use image::DynamicImage;
use std::collections::HashMap;

/// This is the panel rendered to the right of the screen which is responsible for showing details
/// about an entry. It includes a few TextViews and a canvas view used for rendering the book's
/// cover.
///
/// # Arguments
///
/// * `width` - Initial size of the panel.
///
pub fn side_panel(width: usize) -> Panel<PaddedView<ScrollView<LinearLayout>>> {
    let canvas =
        HideableView::new(CanvasView::new(Vec2::new(width / 3, 10))).with_name("side_panel_canvas");

    let padding_left = ResizedView::with_full_width(DummyView::new());
    let padding_right = ResizedView::with_full_width(DummyView::new());

    let canvas_layer = LinearLayout::horizontal()
        .child(padding_left)
        .child(canvas)
        .child(padding_right);

    let mut title = TextView::new("").with_name("side_panel_title");
    let mut author = TextView::new("").with_name("side_panel_author");

    let details = TextView::new("").with_name("side_panel_details");

    title.get_mut().set_style(cursive::theme::Effect::Bold);
    author.get_mut().set_style(cursive::theme::Effect::Italic);

    let layout = LinearLayout::vertical()
        .child(title)
        .child(author)
        .child(canvas_layer)
        .child(details);
    // returns the entire thing as a layout
    Panel::new(PaddedView::lrtb(
        2,
        2,
        0,
        0,
        ScrollView::new(layout).scroll_y(true),
    ))
}

/// Updates the side panel with the contents of an entry.
///
/// # Arguments
///
/// * `s` - Reference to cursive instance.
/// * `entry` - Entry to render.
///
pub fn render_entry_in_side_panel(s: &mut Cursive, entry: &EntryType) {
    let mut title = s.find_name::<TextView>("side_panel_title").unwrap();
    let mut author_view = s.find_name::<TextView>("side_panel_author").unwrap();
    let mut details = s.find_name::<TextView>("side_panel_details").unwrap();
    let mut canvas_wrapper = s
        .find_name::<HideableView<CanvasView>>("side_panel_canvas")
        .unwrap();

    match entry {
        EntryType::File(fname, url) | EntryType::Directory(fname, url) => {
            title.set_content(fname);
            canvas_wrapper.hide();

            author_view.set_content("");
            details.set_content("");
        }
        EntryType::OPDSEntry(data) => {
            title.set_content(&data.title);

            match &data.author {
                Some(a) => author_view.set_content(a),
                None => author_view.set_content(""),
            }

            details.set_content(&data.details);

            let image_data: &mut HashMap<String, DynamicImage> = s.user_data().unwrap();
            let image = image_data.get(&data.title);
            match image {
                Some(im) => {
                    canvas_wrapper.unhide();
                    let canvas: &mut CanvasView = canvas_wrapper.get_inner_mut();
                    canvas.from_image(im);
                }
                None => {
                    canvas_wrapper.hide();
                }
            }
        }
    }
}
