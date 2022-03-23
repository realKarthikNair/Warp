use crate::ui::action_view::ActionView;
use gettextrs::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib, ResponseType};

use crate::ui::application::WarpApplication;

mod imp {
    use super::*;
    use adw::subclass::prelude::AdwApplicationWindowImpl;
    use std::cell::{Cell, RefCell};

    use crate::config::PersistentConfig;
    use crate::glib::clone;
    use crate::globals;
    use crate::util::UIError;
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
        pub config: RefCell<PersistentConfig>,
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
            if globals::DEBUG_BUILD {
                obj.add_css_class("devel");
            }

            self.config
                .replace(PersistentConfig::from_file().unwrap_or_else(
                clone!(@strong obj => move |err| {
                    obj.connect_visible_notify(move |window| {
                        if window.is_visible() {
                            UIError::new(&gettext!(
                                "Error loading config file '{}', using default config.\nError: {}",
                                PersistentConfig::path().display(),
                                err
                            ))
                            .handle();
                        }
                    });

                    PersistentConfig::default()
                }),
            ));

            obj.load_window_size();
            obj.setup_help_overlay();

            self.send_select_file_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.imp().file_chooser.get().unwrap().show();
                }));

            // Open folder
            let action_open_folder = gio::SimpleAction::new("open-folder", None);
            action_open_folder.connect_activate(clone!(@weak obj => move |_, _| {
                if !obj.action_view_showing() {
                    let imp = obj.imp();
                    imp.stack.set_visible_child_name("send");
                    imp.folder_chooser.get().unwrap().show();
                }
            }));
            obj.add_action(&action_open_folder);

            // Open (send) file
            let action_send = gio::SimpleAction::new("open-file", None);
            action_send.connect_activate(clone!(@weak obj => move |_, _| {
                if !obj.action_view_showing() {
                    let imp = obj.imp();
                    imp.stack.set_visible_child_name("send");
                    imp.file_chooser.get().unwrap().show();
                }
            }));
            obj.add_action(&action_send);

            // Receive file
            let action_send = gio::SimpleAction::new("receive-file", None);
            action_send.connect_activate(clone!(@weak obj => move |_, _| {
                if !obj.action_view_showing() {
                    obj.imp().stack.set_visible_child_name("receive");
                    obj.imp().code_entry.grab_focus();
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
                match response {
                    ResponseType::Accept => {
                        if let Some(file) = chooser.file() {
                            if let Some(path) = file.path() {
                                log::debug!("Picked file: {}", path.display());
                                obj.imp().action_view.send_file(path);
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
            window.save_window_size();
            if let Err(err) = self.config.borrow().save() {
                UIError::new(&gettext!("Error saving configuration file: {}", err)).handle();
            }

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

    fn save_window_size(&self) {
        let imp = self.imp();

        let (width, height) = self.default_size();

        imp.config.borrow_mut().window.width = width;
        imp.config.borrow_mut().window.height = height;
    }

    fn load_window_size(&self) {
        let imp = self.imp();

        let width = imp.config.borrow().window.width;
        let height = imp.config.borrow().window.height;

        self.set_default_size(width, height);
    }

    pub fn receive_file_button(&self) {
        let code = self.imp().code_entry.text();
        self.action_view().receive_file(code.to_string());
    }

    pub fn cancel_any_action(&self) {
        if self.action_view_showing() {
            self.action_view().cancel();
        }
    }

    pub fn action_view_showing(&self) -> bool {
        self.imp().action_view_showing.get()
    }

    pub fn show_action_view(&self) {
        let imp = self.imp();
        imp.action_view_showing.set(true);
        imp.leaflet.navigate(adw::NavigationDirection::Forward);
    }

    pub fn navigate_back(&self) {
        let imp = self.imp();
        imp.action_view_showing.set(false);
        imp.leaflet.navigate(adw::NavigationDirection::Back);
        imp.action_view.show_progress_indeterminate(false);
        imp.code_entry.set_text("");
    }

    pub fn toast_overlay(&self) -> adw::ToastOverlay {
        self.imp().toast_overlay.clone()
    }

    pub fn action_view(&self) -> ActionView {
        self.imp().action_view.clone()
    }
}

impl Default for WarpApplicationWindow {
    fn default() -> Self {
        WarpApplication::default().main_window()
    }
}
