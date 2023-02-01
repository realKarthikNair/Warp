use crate::globals;
use crate::ui::application::WarpApplication;
use adw::prelude::*;
use adw::subclass::prelude::*;

mod imp {
    use super::*;
    use gtk::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(file = "welcome_window.ui")]
    pub struct WelcomeWindow {
        #[template_child]
        pub(super) leaflet: TemplateChild<adw::Leaflet>,
        #[template_child]
        status_page: TemplateChild<adw::StatusPage>,
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
            klass.bind_template();
            klass.bind_template_instance_callbacks();
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for WelcomeWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.status_page.set_icon_name(Some(globals::APP_ID));
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

#[gtk::template_callbacks]
impl WelcomeWindow {
    pub fn new() -> Self {
        glib::Object::new(&[])
    }

    fn app(&self) -> WarpApplication {
        WarpApplication::default()
    }

    #[template_callback]
    pub fn navigate_back(&self) {
        self.imp().leaflet.navigate(adw::NavigationDirection::Back);
    }

    #[template_callback]
    pub fn navigate_forward(&self) {
        self.imp()
            .leaflet
            .navigate(adw::NavigationDirection::Forward);
    }

    #[template_callback]
    pub fn get_started_button_clicked(&self) {
        self.app().main_window().set_welcome_window_shown(true);
        self.close();
    }
}

impl Default for WelcomeWindow {
    fn default() -> Self {
        Self::new()
    }
}
