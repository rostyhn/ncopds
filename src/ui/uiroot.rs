use crate::model::{get_title_for_entry, EntryType};
use crate::server::Server;
use crate::ui;
use crate::ui::canvas::CanvasView;
use crate::ui::dialogs::{input_dialog, notification};
use crate::ui::directory_view::directory_view;
use crate::ui::side_panel::side_panel;
use cursive::reexports::log::{log, Level};
use cursive::view::{Nameable, SizeConstraint};
use cursive::views::{
    Dialog, HideableView, LinearLayout, NamedView, PaddedView, Panel, ResizedView, ScrollView,
    SelectView, TextContent, TextView,
};
use cursive::Cursive;

use std::sync::mpsc;
use std::sync::Arc;

use crate::ControllerMessage;
use image::DynamicImage;
use std::collections::HashMap;
use termsize;

pub struct UIRoot {
    pub cursive: cursive::CursiveRunner<Cursive>,
    ui_rx: mpsc::Receiver<UIMessage>,
    pub ui_tx: mpsc::Sender<UIMessage>,
    controller_tx: mpsc::Sender<ControllerMessage>,
    /// width of screen; used for resizing
    width: usize,
    /// height of screen; used for resizing
    height: usize,
    notifications: Vec<(u32, String)>,
}

#[derive(Debug)]
pub enum UIMessage {
    /// populates the View and Edit trees with a new connection
    AddConnection(String, Server, Option<String>),
    /// changes the entries rendered inside the left panel
    UpdateDirectoryView(String, Vec<EntryType>, String),
    /// shows a dialog box with a title and message
    ShowInfo(String, String),
    /// opens a small menu with entries labeled with the string and hooked up to a controller event
    ShowContextMenu(String, Vec<(String, ControllerMessage)>),
    /// saves an image into memory for display
    StoreImage(String, DynamicImage),
    /// shows a password prompt which updates the password for a given server
    PasswordPrompt(String, Server),
    /// displays a small popup in the bottom right corner of the screen with a given title and
    /// content
    ShowNotification(String, String),
}

impl UIRoot {
    /// Initializes the UI. The screen is divided into two panels, similar to ranger or midnight
    /// commander. The left panel shows the contents of the directory / OPDS page.
    /// The right panel shows details about the currently selected entry in the left panel.
    ///
    /// # Arguments
    ///
    /// * `controller_tx` - Message channel to controller
    /// * `theme_path` - Path to theme file
    /// * `t_size` - terminal size
    ///
    pub fn new(
        controller_tx: mpsc::Sender<ControllerMessage>,
        theme_path: &std::path::Path,
        t_size: termsize::Size,
    ) -> UIRoot {
        let mut cursive =
            cursive::CursiveRunner::new(Cursive::new(), cursive::backends::try_default().unwrap());

        // UI refreshes on its own so you don't have to hit the keys
        cursive.set_autorefresh(true);

        // only show info
        cursive::logger::set_external_filter_level(cursive::reexports::log::LevelFilter::Info);
        cursive::logger::set_internal_filter_level(cursive::reexports::log::LevelFilter::Info);
        // init logger
        cursive::logger::init();

        // load theme
        if theme_path.metadata().is_err() {
            std::fs::File::create(theme_path).expect("failed to create theme file");
        }

        // https://docs.rs/cursive/latest/cursive/theme/index.html
        cursive
            .load_toml(&std::fs::read_to_string(theme_path).expect("could not open theme file"))
            .expect("couldn't read theme");

        let (ui_tx, ui_rx) = mpsc::channel::<UIMessage>();
        let mut ui = UIRoot {
            cursive,
            ui_tx,
            ui_rx,
            controller_tx: controller_tx.clone(),
            width: t_size.cols.into(),
            height: t_size.rows.into(),
            notifications: vec![],
        };

        ui.cursive
            .set_user_data(HashMap::<String, DynamicImage>::new());

        let side_panel = NamedView::new(
            "size_detail_panel",
            ResizedView::with_fixed_width(ui.width / 2, side_panel(ui.width)),
        );

        let file_view = NamedView::new(
            "size_file_view",
            ResizedView::with_fixed_width(ui.width / 2, directory_view(controller_tx.clone())),
        );

        let main_view = ResizedView::new(
            SizeConstraint::Full,
            SizeConstraint::Full,
            LinearLayout::horizontal()
                .child(file_view)
                .child(side_panel),
        );

        ui.cursive.add_fullscreen_layer(main_view);
        ui.cursive.add_global_callback('q', Cursive::quit);
        ui.cursive
            .add_global_callback('~', Cursive::toggle_debug_console);

        ui.cursive.add_global_callback('?', move |s| {
            let d = about_screen();
            s.add_layer(d);
        });

        let search_ctx = controller_tx.clone();
        ui.cursive.add_global_callback('/', move |s| {
            let ss = search_ctx.clone();
            let d = input_dialog(
                "Search",
                move |query| {
                    ss.send(ControllerMessage::Search(query))
                        .expect("Failed to search server.");
                },
                false,
            );
            s.add_layer(d);
        });

        let backctx = controller_tx.clone();
        ui.cursive
            .add_global_callback(cursive::event::Key::Backspace, move |s| {
                // check if popup is open first
                if s.find_name::<SelectView<ControllerMessage>>("popup")
                    .is_some()
                {
                    s.pop_layer();
                } else {
                    backctx.clone().send(ControllerMessage::GoBack()).unwrap();
                }
            });

        let add_ctx = controller_tx.clone();
        let local_ctx = controller_tx.clone();

        // adding a delimiter to the menu bar crashes it?
        ui.cursive
            .menubar()
            .add_leaf("ncopds", |s| {
                let d = about_screen();
                s.add_layer(d);
            })
            .add_subtree(
                "View",
                cursive::menu::Tree::new()
                    .leaf("Download directory", move |_| {
                        local_ctx
                            .send(ControllerMessage::ChangeConnection("local".to_string()))
                            .expect("local connection disappeared");
                    })
                    .leaf("Add connection", move |s| {
                        let diag = ui::serverinfomodal::new(add_ctx.clone());
                        s.add_layer(diag);
                    })
                    .delimiter(),
            )
            .add_subtree("Edit", cursive::menu::Tree::new());
        ui.cursive.set_autohide_menu(false);

        ui
    }

    /// If width / height are different from what is stored inside the UIRoot struct, update the
    /// views accordingly.
    ///
    /// # Arguments
    ///
    /// * `width` - New width
    /// * `height` - New height
    ///
    fn update_size(&mut self, width: usize, height: usize) {
        if self.width != width || self.height != height {
            let file_view = self
                .cursive
                .find_name::<ResizedView<Panel<PaddedView<LinearLayout>>>>("size_file_view");

            let details_panel = self.cursive.find_name::<ResizedView<
                Panel<PaddedView<ScrollView<LinearLayout>>>,
            >>("size_detail_panel");

            if let Some(mut fv) = file_view {
                fv.set_width(SizeConstraint::Fixed(width / 2));
            }

            if let Some(mut dp) = details_panel {
                dp.set_width(SizeConstraint::Fixed(width / 2));
            }

            self.width = width;
            self.height = height;
        }
    }

    /// Main UI loop. Listens to messages from controller and updates UI accordingly.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame we are currently on
    ///
    pub fn step(&mut self, frame: u32) -> bool {
        if !self.cursive.is_running() {
            return false;
        }

        let layer_sizes = self.cursive.screen().layer_sizes();
        let screen_size = layer_sizes.first().unwrap();

        while let Some(message) = self.ui_rx.try_iter().next() {
            match message {
                UIMessage::UpdateDirectoryView(title, items, msg) => {
                    // refactor such that directory view is a struct that can access its fields
                    // directly
                    let mut select = self
                        .cursive
                        .find_name::<SelectView<EntryType>>("file_view")
                        .unwrap();

                    let mut title_view = self.cursive.find_name::<TextView>("title_view").unwrap();
                    let mut msg_view = self.cursive.find_name::<TextView>("file_msg_view").unwrap();
                    msg_view.set_content(&msg);

                    if msg.is_empty() && items.is_empty() {
                        msg_view.set_content("No files found.");
                    }

                    select.clear();
                    for entry in items {
                        let d = entry.clone();
                        match entry {
                            EntryType::File(title, url) => select.add_item(title, d),
                            EntryType::Directory(title, url) => select.add_item(title, d),
                            EntryType::OPDSEntry(e) => select.add_item(&e.title, d),
                        }
                    }

                    title_view.set_content(&title);

                    if !select.is_empty() {
                        let cb = select.set_selection(0);
                        cb(&mut self.cursive);
                    }
                }
                UIMessage::AddConnection(name, server, pwd) => {
                    // update view tree
                    let mb = self.cursive.menubar();
                    let st = mb.get_subtree(1).expect("View tree missing!");

                    let leaf = st.find_item(&name);

                    if leaf.is_none() {
                        let data = name.clone();
                        let ctx = self.controller_tx.clone();

                        st.add_leaf(name.clone(), move |_| {
                            ctx.send(ControllerMessage::ChangeConnection(data.clone()))
                                .expect("Failed to change to new connection");
                        });
                    }

                    // update edit tree
                    let edit_ctx = self.controller_tx.clone();
                    let et = mb.get_subtree(2).expect("Edit tree missing!");

                    let edit_leaf = et.find_item(&name);
                    if edit_leaf.is_none() {
                        et.add_leaf(name.clone(), move |s| {
                            let diag = ui::serverinfomodal::new(edit_ctx.clone());
                            s.add_layer(diag);
                            ui::serverinfomodal::populate_fields(s, &name, &server, pwd.clone());
                        });
                    }
                }
                UIMessage::ShowInfo(title, err) => {
                    // remove any lingering dialogs before showing this one
                    let old_diag = self.cursive.find_name::<Dialog>("info_dialog");

                    if old_diag.is_some() {
                        self.cursive.pop_layer();
                    }

                    let dialog = Dialog::info(&err).title(title).with_name("info_dialog");
                    self.cursive.add_layer(dialog);
                }
                UIMessage::ShowNotification(title, content) => {
                    let id = notification(&mut self.cursive, &title, &content, screen_size);
                    self.notifications.push((frame, id));
                }
                UIMessage::ShowContextMenu(title, entries) => {
                    let ctx = self.controller_tx.clone();
                    let d_ctx = self.controller_tx.clone();

                    let mut select = SelectView::<ControllerMessage>::new().on_submit(
                        move |s, item| match item {
                            ControllerMessage::Rename(old, _) => {
                                s.pop_layer();
                                let dd_ctx = d_ctx.clone();
                                let c_old = old.clone();
                                let dialog = input_dialog(
                                    "Rename file",
                                    move |new_name| {
                                        dd_ctx
                                            .send(ControllerMessage::Rename(
                                                c_old.clone(),
                                                new_name.into(),
                                            ))
                                            .expect("Failed to send rename action");
                                    },
                                    false,
                                );
                                s.add_layer(dialog);
                            }
                            other => {
                                ctx.send(other.clone()).expect("failed to send action");
                                s.pop_layer();
                            }
                        },
                    );

                    for e in entries {
                        select.add_item(e.0, e.1);
                    }

                    self.cursive
                        .add_layer(Dialog::around(NamedView::new("popup", select)).title(&title));
                }
                UIMessage::StoreImage(title, image_data) => {
                    let select = self
                        .cursive
                        .find_name::<SelectView<EntryType>>("file_view")
                        .unwrap();

                    // updates the currently selected entry with the image if we have loaded it in
                    // not the most elegant solution, but it works
                    let selected: Arc<EntryType> = select.selection().unwrap();
                    let selected_title = get_title_for_entry(&selected);
                    if selected_title == title {
                        let mut canvas_wrapper = self
                            .cursive
                            .find_name::<HideableView<CanvasView>>("side_panel_canvas")
                            .unwrap();
                        canvas_wrapper.unhide();

                        let canvas: &mut CanvasView = canvas_wrapper.get_inner_mut();
                        canvas.from_image(&image_data);
                    }

                    self.cursive
                        .with_user_data(|id: &mut HashMap<String, DynamicImage>| {
                            id.insert(title.clone(), image_data.clone())
                        });
                }
                UIMessage::PasswordPrompt(name, s) => {
                    let ctx = self.controller_tx.clone();
                    let server = s.clone();
                    let title = format!(
                        "Please enter a password for {}@{}",
                        s.username.unwrap(),
                        s.base_url
                    );

                    let d = input_dialog(
                        &title,
                        move |pwd| {
                            ctx.send(ControllerMessage::AddConnection(
                                name.clone(),
                                server.clone(),
                                Some(pwd.to_string()),
                            ))
                            .expect("Failed to update connection");
                        },
                        true,
                    );

                    self.cursive.add_layer(d);
                }
            }
        }

        // clears lingering notifications after 5 seconds
        let screen = self.cursive.screen_mut(); // reference to StackView
        for (last_rendered, n_id) in &self.notifications {
            // fps * time in seconds
            if frame - last_rendered > 30 * 5 {
                let pos = screen.find_layer_from_name(n_id);
                if let Some(p) = pos {
                    screen.remove_layer(p);
                }
            }
        }

        self.update_size(screen_size.x, screen_size.y);
        self.cursive.step();
        true
    }
}

fn about_screen() -> Dialog {
    let tc = TextContent::new(
                    "ncopds: A TUI program for OPDS catalogs\n\nHotkeys:\no - Open file in local view mode\nd - Delete file in local view mode\nr - Rename file in local view mode\n/ - Open search if connection supports it\n? - Opens this screen\n Rostyslav Hnatyshyn 2023-2024",
                );
    Dialog::new()
        .title("About ncopds")
        .content(TextView::new_with_content(tc))
        .button("Ok", move |s| {
            s.pop_layer();
        })
}
