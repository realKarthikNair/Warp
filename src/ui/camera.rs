use std::{os::fd::RawFd, sync::Once};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib::{self, clone};
use once_cell::sync::Lazy;

use crate::gettext::*;
use crate::ui::camera_row::CameraRow;
use crate::util::{error::*, future::spawn_async};

mod imp {
    use std::cell::RefCell;

    use glib::subclass::{InitializingObject, Signal};

    use crate::{ui::application::WarpApplication, util::future::main_async_local};

    use super::*;

    #[derive(gtk::CompositeTemplate, Default)]
    #[template(file = "camera.ui")]
    pub struct Camera {
        #[template_child]
        pub toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub viewfinder_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub selection_button: TemplateChild<gtk::MenuButton>,
        pub selection: gtk::SingleSelection,
        pub provider: RefCell<Option<aperture::DeviceProvider>>,
        pub viewfinder: aperture::Viewfinder,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Camera {
        const NAME: &'static str = "Camera";
        type Type = super::Camera;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("camera");
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Camera {
        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("code-detected")
                    .param_types([String::static_type()])
                    .run_first()
                    .build()]
            });
            SIGNALS.as_ref()
        }

        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            self.viewfinder.set_detect_codes(true);
            self.viewfinder
                .connect_state_notify(glib::clone!(@weak obj => move |_| {
                    obj.update_viewfinder_state();
                }));
            obj.update_viewfinder_state();

            self.viewfinder.connect_code_detected(
                glib::clone!(@weak obj => move|_, code_type, code| {
                    if matches!(code_type, aperture::CodeType::Qr) {
                        obj.emit_by_name::<()>("code-detected", &[&code]);
                    }
                }),
            );

            let popover = gtk::Popover::new();
            popover.add_css_class("menu");

            self.selection
                .set_model(Some(aperture::DeviceProvider::instance()));
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, item| {
                let camera_row = CameraRow::default();

                item.downcast_ref::<gtk::ListItem>()
                    .unwrap()
                    .set_child(Some(&camera_row));
            });
            let selection = &self.selection;
            factory.connect_bind(glib::clone!(@weak selection => move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().unwrap();
                let child = item.child().unwrap();
                let row = child.downcast_ref::<CameraRow>().unwrap();

                let item = item.item().and_downcast::<aperture::Camera>().unwrap();
                row.set_label(&item.display_name());

                selection.connect_selected_item_notify(glib::clone!(@weak row, @weak item => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        row.set_selected(selected_item == item);
                    } else {
                        row.set_selected(false);
                    }
                }));
            }));
            let list_view = gtk::ListView::new(Some(self.selection.clone()), Some(factory));
            popover.set_child(Some(&list_view));

            self.selection.connect_selected_item_notify(
                glib::clone!(@weak obj, @weak popover => move |selection| {
                    if let Some(selected_item) = selection.selected_item() {
                        let camera = selected_item.downcast_ref::<aperture::Camera>();
                        obj.imp().viewfinder.set_camera(camera);
                    }
                    popover.popdown();
                }),
            );

            self.selection_button.set_popover(Some(&popover));
        }
    }

    impl WidgetImpl for Camera {}
    impl NavigationPageImpl for Camera {
        fn hidden(&self) {
            self.viewfinder_bin.set_child(gtk::Widget::NONE);
        }

        fn showing(&self) {
            let camera = self.obj();
            main_async_local(
                glib::clone!(@strong camera => move |err| camera.imp().handle_error(&err)),
                glib::clone!(@strong camera => async move {
                    camera.scan_from_camera().await
                }),
            );
        }
    }

    #[gtk::template_callbacks]
    impl Camera {
        #[template_callback]
        fn on_troubleshooting_clicked(&self) {
            WarpApplication::default().activate_action("help", None);
        }

        fn handle_error(&self, error: &AppError) {
            let description = if let AppError::Ashpd {
                source: ashpd::Error::Portal(ashpd::PortalError::NotAllowed(..)),
            } = error
            {
                gettextf(
                    "Camera access denied. Open Settings and allow Warp to access the camera.",
                    &[&error.gettext_error()],
                )
            } else {
                gettextf("Failed to start the camera: {}", &[&error.gettext_error()])
            };

            self.error_page.set_description(Some(&description));
            self.stack.set_visible_child_name("error");
        }
    }
}

glib::wrapper! {
    pub struct Camera(ObjectSubclass<imp::Camera>)
        @extends gtk::Widget, adw::NavigationPage;
}

impl Camera {
    pub fn connect_code_detected<F>(&self, callback: F) -> glib::SignalHandlerId
    where
        F: Fn(&Self, String) + 'static,
    {
        self.connect_local(
            "code-detected",
            false,
            clone!(@weak self as camera => @default-return None, move |args| {
                let code = args[1].get::<String>().unwrap();
                callback(&camera, code);
                None
            }),
        )
    }

    async fn scan_from_camera(&self) -> Result<(), AppError> {
        static INIT: Once = Once::new();

        self.imp()
            .viewfinder_bin
            .set_child(Some(&self.imp().viewfinder));

        if INIT.is_completed() {
            return Ok(());
        }

        let provider = aperture::DeviceProvider::instance();
        match spawn_async(stream()).await {
            Ok(fd) => {
                if let Err(err) = provider.set_fd(fd) {
                    Err(
                        UiError::new(&gettextf("Could not use the camera portal: {}", &[&err]))
                            .into(),
                    )
                } else if let Err(err) = provider.start_with_default(|camera| {
                    matches!(camera.location(), aperture::CameraLocation::Back)
                }) {
                    Err(UiError::new(&gettextf(
                        "Could not start the device provider: {}",
                        &[&err],
                    ))
                    .into())
                } else {
                    log::debug!("Device provider started");
                    INIT.call_once(|| ());
                    Ok(())
                }
            }
            Err(err) => Err(err),
        }
    }

    pub fn stop(&self) {
        log::debug!("Stopping camera");
        self.imp().viewfinder_bin.set_child(gtk::Widget::NONE);
    }

    fn update_viewfinder_state(&self) {
        let imp = self.imp();
        let state = imp.viewfinder.state();
        match state {
            aperture::ViewfinderState::Loading => {
                imp.stack.set_visible_child_name("loading");
            }
            aperture::ViewfinderState::Error | aperture::ViewfinderState::NoCameras => {
                imp.stack.set_visible_child_name("not-found");
            }
            aperture::ViewfinderState::Ready => {
                imp.stack.set_visible_child_name("stream");
            }
        }
        log::info!("The camera state changed: {state:?}");

        let is_ready = matches!(state, aperture::ViewfinderState::Ready);
        self.imp()
            .toolbar_view
            .set_extend_content_to_top_edge(is_ready);
        if is_ready {
            self.imp().toolbar_view.add_css_class("extended");
        } else {
            self.imp().toolbar_view.remove_css_class("extended");
        }

        if matches!(state, aperture::ViewfinderState::Loading) {
            imp.spinner.start();
        } else {
            imp.spinner.stop();
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        glib::Object::new()
    }
}

async fn stream() -> ashpd::Result<RawFd> {
    let proxy = ashpd::desktop::camera::Camera::new().await?;
    proxy.request_access().await?;

    proxy.open_pipe_wire_remote().await
}
