use crate::ui::action_view::ActionView;
use gettextrs::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, ResponseType};

use crate::ui::application::WarpApplication;

mod imp {
    use super::*;
    use adw::subclass::prelude::AdwApplicationWindowImpl;
    use std::cell::Cell;

    use crate::glib::clone;
    use crate::globals;
    use gtk::CompositeTemplate;
    use once_cell::sync::OnceCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/net/felinira/warp/ui/window.ui")]
    pub struct WarpApplicationWindow {
        #[template_child]
        pub stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub headerbar: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub leaflet: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub send_select_file_button: TemplateChild<adw::SplitButton>,
        #[template_child]
        pub receive_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub code_entry: TemplateChild<gtk::Entry>,
        pub action_view: ActionView,
        pub file_chooser: OnceCell<gtk::FileChooserNative>,
        pub folder_chooser: OnceCell<gtk::FileChooserNative>,
        pub action_view_showing: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WarpApplicationWindow {
        const NAME: &'static str = "WarpApplicationWindow";
        type Type = super::WarpApplicationWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WarpApplicationWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Devel Profile
            if globals::PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            obj.setup_help_overlay();

            self.send_select_file_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    imp::WarpApplicationWindow::from_instance(&obj).file_chooser.get().unwrap().show();
                }));

            // Open folder
            let action_open_folder = gio::SimpleAction::new("open-folder", None);
            action_open_folder.connect_activate(clone!(@weak obj => move |_, _| {
                if !obj.action_view_showing() {
                    let obj_ = imp::WarpApplicationWindow::from_instance(&obj);
                    obj_.stack.set_visible_child_name("send");
                    obj_.folder_chooser.get().unwrap().show();
                }
            }));
            obj.add_action(&action_open_folder);

            // Open (send) file
            let action_send = gio::SimpleAction::new("open-file", None);
            action_send.connect_activate(clone!(@weak obj => move |_, _| {
                if !obj.action_view_showing() {
                    let obj_ = imp::WarpApplicationWindow::from_instance(&obj);
                    obj_.stack.set_visible_child_name("send");
                    obj_.file_chooser.get().unwrap().show();
                }
            }));
            obj.add_action(&action_send);

            // Receive file
            let action_send = gio::SimpleAction::new("receive-file", None);
            action_send.connect_activate(clone!(@weak obj => move |_, _| {
                if !obj.action_view_showing() {
                    let obj_ = imp::WarpApplicationWindow::from_instance(&obj);
                    obj_.stack.set_visible_child_name("receive");
                    obj_.code_entry.grab_focus();
                }
            }));
            obj.add_action(&action_send);

            self.receive_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.receive_file_button();
                }));

            self.code_entry
                .connect_activate(clone!(@weak obj => move |_| {
                    obj.receive_file_button();
                }));

            self.leaflet.append(&self.action_view);

            let file_chooser_closure = clone!(@strong obj => move |chooser: &gtk::FileChooserNative, response: gtk::ResponseType| {
                let self_ = imp::WarpApplicationWindow::from_instance(&obj);
                match response {
                    ResponseType::Accept => {
                        if let Some(file) = chooser.file() {
                            if let Some(path) = file.path() {
                                log::debug!("Picked file: {}", path.display());
                                self_.action_view.send_file(path);
                            } else {
                                log::error!("File chooser has file but path is None")
                            }
                        } else {
                            log::debug!("File chooser accepted but no file selected");
                        }
                    }
                    ResponseType::Cancel => {
                        log::debug!("File chooser canceled");
                    }
                    _ => {
                        log::error!("Unknown file chooser response type");
                    }
                };
            });

            let file_chooser = self.file_chooser.get_or_init(move || {
                gtk::FileChooserNative::new(
                    Some(&gettext("Select file to send")),
                    Some(obj),
                    gtk::FileChooserAction::Open,
                    Some(&gettext("Open")),
                    Some(&gettext("Cancel")),
                )
            });

            file_chooser.set_modal(true);
            file_chooser.set_transient_for(Some(obj));
            file_chooser.connect_response(file_chooser_closure.clone());

            let folder_chooser = self.folder_chooser.get_or_init(move || {
                gtk::FileChooserNative::new(
                    Some(&gettext("Select folder to send")),
                    Some(obj),
                    gtk::FileChooserAction::SelectFolder,
                    Some(&gettext("Open Folder")),
                    Some(&gettext("Cancel")),
                )
            });

            folder_chooser.set_modal(true);
            folder_chooser.set_transient_for(Some(obj));
            folder_chooser.connect_response(file_chooser_closure);

            self.code_entry.connect_has_focus_notify(|entry| {
                // Select all text when entry is focused
                entry.select_region(0, -1);
            });
        }
    }

    impl WidgetImpl for WarpApplicationWindow {}
    impl WindowImpl for WarpApplicationWindow {
        // Save window state on delete event
        fn close_request(&self, window: &Self::Type) -> gtk::Inhibit {
            // Pass close request on to the parent
            self.parent_close_request(window)
        }
    }

    impl ApplicationWindowImpl for WarpApplicationWindow {}
    impl AdwApplicationWindowImpl for WarpApplicationWindow {}
}

glib::wrapper! {
    pub struct WarpApplicationWindow(ObjectSubclass<imp::WarpApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl WarpApplicationWindow {
    pub fn new(app: &WarpApplication) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to create WarpApplicationWindow")
    }

    fn setup_help_overlay(&self) {
        let builder = gtk::Builder::from_resource("/net/felinira/warp/ui/help_overlay.ui");
        let shortcuts: Option<gtk::ShortcutsWindow> = builder.object("help_overlay");
        self.set_help_overlay(shortcuts.as_ref());
    }

    pub fn receive_file_button(&self) {
        let code = &imp::WarpApplicationWindow::from_instance(self)
            .code_entry
            .text();
        self.action_view().receive_file(code.to_string());
    }

    pub fn cancel_any_action(&self) {
        if self.action_view_showing() {
            self.action_view().cancel();
        }
    }

    pub fn action_view_showing(&self) -> bool {
        imp::WarpApplicationWindow::from_instance(self)
            .action_view_showing
            .get()
    }

    pub fn show_action_view(&self) {
        let self_ = imp::WarpApplicationWindow::from_instance(self);
        self_.action_view_showing.set(true);
        self_.leaflet.navigate(adw::NavigationDirection::Forward);
    }

    pub fn navigate_back(&self) {
        let self_ = imp::WarpApplicationWindow::from_instance(self);
        self_.action_view_showing.set(false);
        self_.leaflet.navigate(adw::NavigationDirection::Back);
        self_.action_view.show_progress_indeterminate(false);
        self_.code_entry.set_text("");
    }

    pub fn toast_overlay(&self) -> adw::ToastOverlay {
        imp::WarpApplicationWindow::from_instance(self)
            .toast_overlay
            .clone()
    }

    pub fn action_view(&self) -> ActionView {
        imp::WarpApplicationWindow::from_instance(self)
            .action_view
            .clone()
    }
}

impl Default for WarpApplicationWindow {
    fn default() -> Self {
        WarpApplication::default().main_window()
    }
}
