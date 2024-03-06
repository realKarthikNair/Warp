use std::os::fd::OwnedFd;
use std::pin::pin;
use std::sync::Once;

use adw::prelude::*;
use adw::subclass::prelude::*;
use futures::FutureExt;
use gtk::glib::{self, clone};
use once_cell::sync::Lazy;

use crate::gettext::*;
use crate::ui::camera_row::CameraRow;
use crate::util::error::*;

mod imp {
    use std::{
        cell::{Cell, OnceCell, RefCell},
        collections::HashMap,
        os::fd::FromRawFd,
        sync::atomic::AtomicU32,
    };

    use glib::subclass::{InitializingObject, Signal};

    use crate::{globals, ui::window::WarpApplicationWindow, util::future::main_async_local};

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
        pub viewfinder: OnceCell<aperture::Viewfinder>,

        pub portal_cancellable: RefCell<Option<gio::Cancellable>>,
        pub use_hack: Cell<bool>,
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
    }

    impl WidgetImpl for Camera {}
    impl NavigationPageImpl for Camera {
        fn hidden(&self) {
            if let Some(viewfinder) = self.viewfinder.get() {
                viewfinder.stop_stream();
            }
        }

        fn showing(&self) {
            log::debug!("Showing camera");
            self.init();

            let camera = self.obj();
            main_async_local(
                glib::clone!(@weak camera => move |err| camera.imp().handle_error(&err)),
                glib::clone!(@strong camera => async move {
                    camera.scan_from_camera().await
                }),
            );
        }
    }

    #[gtk::template_callbacks]
    impl Camera {
        pub(super) fn init(&self) {
            static INIT: Once = Once::new();

            let obj = self.obj();

            INIT.call_once(move || {
                log::debug!("Initializing aperture");
                aperture::init(globals::APP_ID);

                log::debug!("Creating viewfinder");
                let viewfinder = self.viewfinder.get_or_init(aperture::Viewfinder::new);
                viewfinder.set_detect_codes(true);

                viewfinder.connect_state_notify(glib::clone!(@weak obj => move |viewfinder| {
                    obj.update_viewfinder_state(viewfinder);
                }));
                obj.update_viewfinder_state(viewfinder);

                viewfinder.connect_code_detected(glib::clone!(@weak obj => move|_, code_type, code| {
                    if matches!(code_type, aperture::CodeType::Qr) {
                        obj.emit_by_name::<()>("code-detected", &[&code]);
                    }
                }));

                let popover = gtk::Popover::new();
                popover.add_css_class("menu");

                log::debug!("Creating device provider");
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
                            obj.imp().viewfinder.get().unwrap().set_camera(camera);
                        }
                        popover.popdown();
                    }),
                );

                self.selection_button.set_popover(Some(&popover));

                log::debug!("Showing camera widget");
                self.viewfinder_bin.set_child(Some(viewfinder));

                log::debug!("Camera init done");
            });
        }

        #[template_callback]
        fn on_retry_clicked(&self) {
            let camera = self.obj();
            main_async_local(
                glib::clone!(@weak camera => move |err| camera.imp().handle_error(&err)),
                glib::clone!(@strong camera => async move {
                    camera.scan_from_camera().await
                }),
            );
        }

        #[template_callback]
        fn on_troubleshooting_clicked(&self) {
            if let Some(app) = self
                .obj()
                .root()
                .and_downcast::<WarpApplicationWindow>()
                .and_then(|window| window.app())
            {
                app.open_help(Some("details-qr"));
            }
        }

        fn handle_error(&self, error: &AppError) {
            log::error!("Camera error: {}", error);
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

            self.error_page.set_title(&gettext("Error"));
            self.error_page.set_description(Some(&description));
            self.stack.set_visible_child_name("error");
        }

        /// This is *really* bad code. We only use it as a last resort if the ashpd zbus connection
        /// is permanently broken beyond repair due to https://github.com/flatpak/xdg-dbus-proxy/issues/46
        async fn request_permission_hack(&self) -> Result<OwnedFd, AppError> {
            static REQUEST_ID: AtomicU32 = AtomicU32::new(1);

            let generic_err =
                || AppError::from(UiError::new(&gettext("Error talking to the camera portal")));

            log::warn!("Resorting to fallback portal code");

            // Create desktop proxy
            let proxy = gio::DBusProxy::for_bus_future(
                gio::BusType::Session,
                gio::DBusProxyFlags::NONE,
                None,
                "org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Camera",
            )
            .await?;

            // Create new request token
            let sender = proxy
                .connection()
                .unique_name()
                .map(|name| String::from(&name.as_str()[1..].replace('.', "_")))
                .ok_or_else(generic_err)?;
            let token = format!(
                "warpportalhack{}",
                REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            );

            // The path to subscribe to for responses
            let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");

            // Response signal
            let (response_sender, response_receiver) = async_channel::bounded::<u32>(1);

            // Subscribe to the response signal
            let connection = proxy.connection();
            let response_subscription = connection.signal_subscribe(
                None,
                Some("org.freedesktop.portal.Request"),
                Some("Response"),
                Some(&request_path),
                None,
                gio::DBusSignalFlags::NO_MATCH_RULE,
                move |_connection, _sender, _path, _interface, _signal, params| {
                    let response = if let Some((response, _data)) =
                        params.get::<(u32, HashMap<String, glib::Variant>)>()
                    {
                        response
                    } else {
                        u32::MAX
                    };

                    // Send the response
                    if let Err(err) = response_sender.try_send(response) {
                        log::error!("{}", err);
                    }
                },
            );

            // Send portal request
            log::debug!("Call AccessCamera portal");

            // Send the token that we listen to above
            let mut options = HashMap::<&str, glib::Variant>::new();
            options.insert("handle_token", token.to_variant());
            let request_handle = proxy
                .call_future(
                    "AccessCamera",
                    Some(&(options,).to_variant()),
                    gio::DBusCallFlags::NONE,
                    -1,
                )
                .await?;

            // Compare the handle with what we precomputed
            if let Ok((request_handle,)) = request_handle.try_get::<(glib::variant::ObjectPath,)>()
            {
                log::debug!("Got handle {:?}", request_handle);
                if request_handle.as_str() != request_path {
                    log::error!(
                        "Expected handle {}, got {}",
                        request_path,
                        request_handle.as_str()
                    );
                    return Err(generic_err());
                }
            } else {
                log::error!("Got {:?}", request_handle);
                return Err(generic_err());
            }

            // Request sent. Now we wait for a response for the provided token
            let response = response_receiver.recv().await?;
            connection.signal_unsubscribe(response_subscription);
            log::debug!("AccessCamera portal response: {}", response);

            if response != 0 {
                log::error!(
                    "Portal request responded with non-zero resopnse code. No camera for us :("
                );
                return Err(UiError::new(&gettext(
                    "Camera access denied. Open Settings and allow Warp to access the camera.",
                ))
                .into());
            }

            log::debug!("Portal request succeeded :)");

            // We are allowed to request an FD now
            log::debug!("Call OpenPipeWireRemote");

            let (result, fd_list) = proxy
                .call_with_unix_fd_list_future(
                    "OpenPipeWireRemote",
                    Some(&(HashMap::<&str, glib::Variant>::new(),).to_variant()),
                    gio::DBusCallFlags::NONE,
                    -1,
                    None::<&gio::UnixFDList>,
                )
                .await?;

            if let Ok((handle,)) = result.try_get::<(glib::variant::Handle,)>() {
                // The handle is an index into the fd list
                let fd = fd_list.get(handle.0)?;
                // Safety: For better or for worse, we trust the camera portal
                Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) })
            } else {
                log::error!("Error getting pipewire FD. Got {:?} instead of (h)", result);
                Err(generic_err())
            }
        }

        pub(super) async fn request_permission_zbus(&self) -> Result<OwnedFd, AppError> {
            if let Some(cancellable) = self.portal_cancellable.take() {
                log::debug!("Canceling last operation");
                cancellable.cancel();
            }

            let cancellable = gio::Cancellable::new();
            self.portal_cancellable.replace(Some(cancellable.clone()));

            let proxy = ashpd::desktop::camera::Camera::new().await?;

            let cancel_fut = pin!(cancellable.future());
            let mut cancel = cancel_fut.fuse();
            let request = pin!(proxy.request_access());
            let mut access_request = request.fuse();
            futures::select! {
                () = cancel => {
                    log::debug!("Canceled");
                    self.portal_cancellable.take();
                    Err(AppError::Canceled)
                },
                res = access_request => Ok(res?)
            }?;

            self.portal_cancellable.take();

            log::debug!("Open PipeWire remote");
            Ok(proxy.open_pipe_wire_remote().await?)
        }

        pub(super) async fn request_permission(&self) -> Result<OwnedFd, AppError> {
            log::debug!("Requesting access to the camera");

            if self.use_hack.get() {
                self.request_permission_hack().await
            } else {
                match self.request_permission_zbus().await {
                    Ok(fd) => Ok(fd),
                    Err(AppError::Ashpd {
                        source:
                            ashpd::Error::Portal(ashpd::PortalError::ZBus(zbus::Error::InputOutput(
                                _io,
                            ))),
                    }) => {
                        self.use_hack.set(true);
                        self.request_permission_hack().await
                    }
                    Err(err) => Err(err),
                }
            }
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

        log::debug!("Starting camera stream");
        if let Some(viewfinder) = self.imp().viewfinder.get() {
            viewfinder.start_stream();
        } else {
            self.imp().init();
        }

        if INIT.is_completed() {
            return Ok(());
        }

        log::debug!("Initializing camera");
        let obj = self.clone();
        let provider = aperture::DeviceProvider::instance();
        match glib::MainContext::default()
            .spawn_local(async move { obj.imp().request_permission().await })
            .await
        {
            Ok(Ok(fd)) => {
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
            Ok(Err(err)) => Err(err),
            Err(glib::JoinError { .. }) => Ok(()),
        }
    }

    pub fn stop(&self) {
        log::debug!("Stopping camera stream");
        if let Some(viewfinder) = self.imp().viewfinder.get() {
            viewfinder.stop_stream();
        }
    }

    fn update_viewfinder_state(&self, viewfinder: &aperture::Viewfinder) {
        let imp = self.imp();
        let state = viewfinder.state();
        match state {
            aperture::ViewfinderState::Loading => {
                imp.stack.set_visible_child_name("loading");
            }
            aperture::ViewfinderState::Error | aperture::ViewfinderState::NoCameras => {
                imp.stack.set_visible_child_name("error");
                imp.error_page.set_title(&gettext("No Camera Found"));
                imp.error_page
                    .set_description(Some(&gettext("Connect a camera to scan QR codes")));
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
