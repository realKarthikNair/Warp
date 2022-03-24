use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use gtk::subclass::prelude::*;

use crate::globals;

mod imp {
    use super::*;
    use adw::NavigationDirection;

    use crate::glib::clone;
    use crate::ui::window::WarpApplicationWindow;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/net/felinira/warp/ui/welcome_window.ui")]
    pub struct WelcomeWindow {
        #[template_child]
        leaflet: TemplateChild<adw::Leaflet>,
        #[template_child]
        status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        close_button: TemplateChild<gtk::Button>,
        #[template_child]
        back_button: TemplateChild<gtk::Button>,
        #[template_child]
        next_button: TemplateChild<gtk::Button>,
        #[template_child]
        get_started_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for WelcomeWindow {
        const NAME: &'static str = "WelcomeWindow";
        type Type = super::WelcomeWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WelcomeWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.status_page.set_icon_name(Some(globals::APP_ID));

            self.close_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.close();
                }));

            self.back_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.imp().leaflet.navigate(NavigationDirection::Back);
                }));

            self.next_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    obj.imp().leaflet.navigate(NavigationDirection::Forward);
                }));

            self.get_started_button
                .connect_clicked(clone!(@weak obj => move |_| {
                    WarpApplicationWindow::default().set_welcome_window_shown(true);
                    obj.close();
                }));
        }
    }

    impl WidgetImpl for WelcomeWindow {}
    impl WindowImpl for WelcomeWindow {}
    impl AdwWindowImpl for WelcomeWindow {}
}

glib::wrapper! {
    pub struct WelcomeWindow(ObjectSubclass<imp::WelcomeWindow>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl WelcomeWindow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create WelcomeWindow")
    }
}
