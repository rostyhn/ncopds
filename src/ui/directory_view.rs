use crate::model::{get_title_for_entry, EntryType};
use crate::ui::dialogs::input_dialog;
use crate::ui::side_panel::render_entry_in_side_panel;
use crate::ControllerMessage;
use cursive::view::Nameable;
use cursive::views::{
    LinearLayout, OnEventView, PaddedView, Panel, ScrollView, SelectView, TextView,
};
use image::DynamicImage;
use std::collections::HashMap;
use std::sync::mpsc;

/// Panel that is rendered to the left of the screen. Renders entries from the currently visited
/// connection. Entries can be selected by clicking on them or pressing enter, which either opens a
/// context menu or navigates to a new page depending on the content of the entry. There are some
/// shortcuts in file mode as well. You can open files with "o", delete them with "d" and rename
/// them with "r". These functions are available inside the context menu as well.
///
/// # Arguments
///
/// * `ctx` - Controller message channel
///
pub fn directory_view(ctx: mpsc::Sender<ControllerMessage>) -> Panel<PaddedView<LinearLayout>> {
    let select_ctx = ctx.clone();
    let submit_ctx = ctx.clone();

    let select = SelectView::<EntryType>::new()
        .on_submit(move |_, item| {
            submit_ctx
                .send(ControllerMessage::EntrySelected(item.clone()))
                .expect("failed to send controller message");
        })
        .on_select(move |s, item| {
            // render the item in the side view
            let image_data: &mut HashMap<String, DynamicImage> = s.user_data().unwrap();
            let image = image_data.get(&get_title_for_entry(item));

            if image.is_none() {
                select_ctx
                    .send(ControllerMessage::RequestImage(item.clone()))
                    .expect("failed to send controller message");
            }
            render_entry_in_side_panel(s, item);
        })
        .with_name("file_view");

    let mut title_view = TextView::new("Title").with_name("title_view");
    title_view.get_mut().set_style(cursive::theme::Effect::Bold);

    let mut msg_view = TextView::new("").with_name("file_msg_view");
    msg_view.get_mut().set_style(cursive::theme::Effect::Italic);
    //mv.h_align(cursive::align::HAlign::Center);

    let file_view = ScrollView::new(select).scroll_x(true);

    let open_ctx = ctx.clone();
    let delete_ctx = ctx.clone();

    // maybe show notification when trying hotkeys on invalid entries?
    let fv = OnEventView::new(file_view)
        .on_event('o', move |s| {
            let select_view = s
                .find_name::<SelectView<EntryType>>("file_view")
                .expect("select view disappeared");

            let binding = select_view.selection().unwrap();
            let item = binding.as_ref();

            if let EntryType::File(_, p) = item {
                open_ctx
                    .send(ControllerMessage::Open(p.clone()))
                    .expect("failed to send controller message");
            }
        })
        .on_event('d', move |s| {
            let select_view = s
                .find_name::<SelectView<EntryType>>("file_view")
                .expect("select view disappeared");

            let binding = select_view.selection().unwrap();
            let item = binding.as_ref();
            match item {
                EntryType::File(_, p) | EntryType::Directory(_, p) => {
                    delete_ctx
                        .send(ControllerMessage::Delete(p.clone()))
                        .expect("failed to send controller message");
                }
                _ => {}
            }
        })
        .on_event('r', move |s| {
            let select_view = s
                .find_name::<SelectView<EntryType>>("file_view")
                .expect("select view disappeared");

            let binding = select_view.selection().unwrap();
            let item = binding.as_ref();
            match item {
                EntryType::File(_, p) | EntryType::Directory(_, p) => {
                    let fp = p.to_file_path().unwrap().clone();

                    let r_ctx = ctx.clone();
                    let d = input_dialog(
                        "Rename file",
                        move |new_name| {
                            r_ctx
                                .send(ControllerMessage::Rename(fp.clone(), new_name.into()))
                                .expect("failed to send controller message");
                        },
                        false,
                    );
                    s.add_layer(d);
                }
                _ => {}
            };
        });

    Panel::new(PaddedView::lrtb(
        2,
        2,
        0,
        0,
        LinearLayout::vertical()
            .child(title_view)
            .child(fv)
            .child(msg_view),
    ))
}
