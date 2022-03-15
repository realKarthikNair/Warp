use crate::glib::clone;
use async_channel;
use async_channel::{Receiver, RecvError, SendError, Sender, TryRecvError};
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict, PyString};
use std::cell::{Cell, Ref, RefCell};
use std::sync::{mpsc, Arc};

use crate::service::twisted::TwistedReactor;
use crate::util;
use crate::{glib, globals};

#[derive(Debug)]
enum WormholeMessage {
    Code(String),
    Message(Vec<u8>),
    Versions,
    Close,
}

impl WormholeMessage {
    pub fn code(str: &str) -> WormholeMessage {
        Self::Code(str.to_string())
    }

    pub fn msg(data: &[u8]) -> WormholeMessage {
        Self::Message(Vec::from(data))
    }
}

#[pyclass]
struct WormholeDelegate {
    tx: Sender<WormholeMessage>,
    closed: Cell<bool>,
}

impl WormholeDelegate {
    fn new(tx: Sender<WormholeMessage>) -> Self {
        Self {
            tx,
            closed: Cell::new(false),
        }
    }

    fn send(&self, msg: WormholeMessage) {
        if !self.closed.get() {
            util::do_async(clone!(@strong self.tx as tx => async move {
                log::debug!("Sending message: {:?}", msg);
                let res = tx.send(msg).await;
                if let Err(e) = res {
                    log::debug!("SendError: {}", e);
                    log::debug!("This is expected if we just closed the wormhole");
                }

                Ok(())
            }));
        }
    }
}

#[pymethods]
impl WormholeDelegate {
    fn wormhole_got_code(&self, code: &str) {
        log::debug!("Code: {}", code);
        self.send(WormholeMessage::code(code));
    }

    fn wormhole_got_unverified_key(&self, key: &[u8]) {
        log::debug!("Got unverified key {:?}", key);
    }

    fn wormhole_got_verifier(&self, verifier: &[u8]) {
        log::debug!("Got verifier {:?}", verifier);
    }

    fn wormhole_got_versions(&self, versions: &PyDict) {
        log::debug!("Got versions {}", versions);
        self.send(WormholeMessage::Versions);
    }

    fn wormhole_got_message(&self, msg: &[u8]) {
        log::debug!("Data: {} bytes", msg.len());
        self.send(WormholeMessage::msg(msg));
    }

    fn wormhole_closed(&self, result: PyObject) {
        Python::with_gil(|py| {
            let result = result.as_ref(py);
            log::debug!("Wormhole closed: {:?}", result);
        });

        self.send(WormholeMessage::Close);
        self.close();
    }

    fn wormhole_got_welcome(&self, welcome: &PyDict) {
        log::debug!("Welcome {}", welcome);
    }

    fn close(&self) {
        self.closed.set(true);
    }
}

#[derive(Clone, Debug)]
pub enum WormholeState {
    Initialized,
    CodePresent,
    Connected,
    Dilated,
    Closed,
}

#[derive(Debug)]
pub struct Wormhole {
    delegate: PyObject,
    wormhole: PyObject,
    endpoints: RefCell<Option<(PyObject, PyObject, PyObject)>>,
    code: RefCell<Option<String>>,
    state: RefCell<WormholeState>,
    rx: Receiver<WormholeMessage>,
}

impl Wormhole {
    pub async fn new() -> PyResult<Wormhole> {
        let (tx, rx) = async_channel::unbounded();

        let res: PyResult<PyObject> = Python::with_gil(|py| {
            let delegate: PyObject = WormholeDelegate::new(tx).into_py(py);
            Ok(delegate)
        });
        let delegate = res?;
        let cloned_reactor = globals::TWISTED_REACTOR.py_reactor().clone();
        let cloned_delegate = delegate.clone();

        let (wormhole_tx, wormhole_rx) = async_channel::bounded(1);
        globals::TWISTED_REACTOR.call_from_thread(move |py| {
            let wormhole = py.import("wormhole")?;
            let kwargs = vec![("delegate", &cloned_delegate)];
            let w = wormhole.call_method(
                "create",
                (
                    "net.felinira.warp",
                    globals::WORMHOLE_RENDEZVOUS_RELAY,
                    &cloned_reactor.into_py(py),
                ),
                Some(kwargs.into_py_dict(py)),
            )?;

            let res = wormhole_tx.try_send(w.into());
            if let Err(err) = res {
                match err {
                    async_channel::TrySendError::Full(_) => panic!("Channel full"),
                    async_channel::TrySendError::Closed(_) => panic!("Channel closed"),
                }
            }

            Ok(())
        })?;

        let wormhole = wormhole_rx.recv().await;
        if let Err(err) = wormhole {
            panic!("Channel closed");
        }

        let wormhole = wormhole.unwrap();

        let instance = Self {
            delegate,
            wormhole,
            endpoints: RefCell::new(None),
            code: RefCell::new(None),
            state: RefCell::new(WormholeState::Initialized),
            rx,
        };

        log::debug!("Wormhole created");
        Ok(instance)
    }

    pub fn allocate_code(&self) -> PyResult<()> {
        let w = self.wormhole.clone();
        globals::TWISTED_REACTOR.call_from_thread(move |py| {
            w.call_method0(py, "allocate_code")?;
            Ok(())
        })
    }

    pub fn get_code(&self) -> Option<String> {
        self.code.borrow().clone()
    }

    pub async fn dilate(&self) -> PyResult<()> {
        let w = self.wormhole.clone();
        let (tx, rx) = async_channel::unbounded();
        globals::TWISTED_REACTOR.call_from_thread(move |py| {
            let kwargs = vec![("transit_relay_location", &globals::WORMHOLE_TRANSIT_RELAY)];
            let endpoints = w.call_method(py, "dilate", (), Some(kwargs.into_py_dict(py)))?;
            let endpoints_tuple: (PyObject, PyObject, PyObject) = endpoints.extract(py)?;
            log::debug!("Got endpoints tuple: {:?}", endpoints_tuple);
            tx.try_send(endpoints_tuple.into()).unwrap();

            Ok(())
        })?;

        self.endpoints.replace(Some(rx.recv().await.unwrap()));

        Ok(())
    }

    fn process_msg(&self, msg: WormholeMessage) -> WormholeState {
        let old_state = self.state.borrow().clone();
        let state = match msg {
            WormholeMessage::Code(code) => {
                self.code.replace(Some(code.clone()));
                match old_state {
                    WormholeState::Initialized => WormholeState::CodePresent,
                    _ => old_state,
                }
            }
            WormholeMessage::Message(_) => match old_state {
                WormholeState::CodePresent => WormholeState::Connected,
                _ => old_state,
            },
            WormholeMessage::Versions => match old_state {
                WormholeState::CodePresent => WormholeState::Connected,
                _ => old_state,
            },
            WormholeMessage::Close => WormholeState::Closed,
        };

        self.state.replace(state.clone());
        log::debug!("New wormhole state: {:?}", state);
        state
    }

    pub fn poll_state(&self) -> WormholeState {
        loop {
            let res = self.rx.try_recv();
            match res {
                Err(err) => match err {
                    TryRecvError::Empty => break,
                    TryRecvError::Closed => {
                        self.state.replace(WormholeState::Closed);
                        return WormholeState::Closed;
                    }
                },
                Ok(msg) => self.process_msg(msg),
            };
        }

        self.state.borrow().clone()
    }

    pub async fn async_state(&self) -> WormholeState {
        let msg = self.rx.recv().await;
        if let Err(err) = &msg {
            self.state.replace(WormholeState::Closed);
            WormholeState::Closed
        } else {
            self.process_msg(msg.unwrap())
        }
    }

    pub fn send_text_message(&self, message: &str) -> PyResult<()> {
        log::debug!("Sending message: {}", message);
        let message = message.to_string();
        let w = self.wormhole.clone();
        globals::TWISTED_REACTOR.call_from_thread(move |py| {
            let json = py.import("json")?;
            let dict = PyDict::new(py);
            let offer = PyDict::new(py);
            offer.set_item("message", message.to_string())?;
            dict.set_item("offer", offer)?;

            log::debug!("{:?}", dict.to_string());
            let bin_dict = json
                .call_method1("dumps", (dict,))?
                .call_method1("encode", ("utf-8",))?;
            let res = w.call_method1(py, "send_message", (bin_dict,));
            if let Err(err) = res {
                err.print(py);
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn close(&self) -> PyResult<()> {
        let w = self.wormhole.clone();
        let d = self.delegate.clone();
        self.rx.close();
        globals::TWISTED_REACTOR.call_from_thread(move |py| {
            d.call_method0(py, "close")?;
            w.call_method0(py, "close")?;
            Ok(())
        })
    }
}
