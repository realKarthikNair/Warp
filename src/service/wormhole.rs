use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict};
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, SendError, Sender};
use std::sync::{mpsc, Arc};

use crate::globals;
use crate::service::twisted::TwistedReactor;

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
}

impl WormholeDelegate {
    fn new(tx: Sender<WormholeMessage>) -> Self {
        Self { tx }
    }

    fn handle_send_err(error: &SendError<WormholeMessage>) {}

    fn send(&self, msg: WormholeMessage) {
        let res = self.tx.send(msg);
        if let Err(e) = res {
            log::error!("SendError: {}", e);
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
        log::debug!("Wormhole closed: {:?}", result);
        self.send(WormholeMessage::Close);
    }

    fn wormhole_got_welcome(&self, welcome: &PyDict) {
        log::debug!("Welcome {}", welcome);
    }
}

pub struct Wormhole {
    reactor: Arc<TwistedReactor>,
    delegate: PyObject,
    wormhole: PyObject,
    code: RefCell<Option<String>>,
    rx: Receiver<WormholeMessage>,
}

impl Wormhole {
    pub fn new(reactor: Arc<TwistedReactor>) -> PyResult<Arc<Wormhole>> {
        let (tx, rx) = mpsc::channel();

        let res: PyResult<PyObject> = Python::with_gil(|py| {
            let delegate: PyObject = WormholeDelegate::new(tx).into_py(py);
            Ok(delegate)
        });
        let delegate = res?;
        let cloned_reactor = reactor.py_reactor().clone();
        let cloned_delegate = delegate.clone();

        let (wormhole_tx, wormhole_rx) = mpsc::channel();
        reactor.call_from_thread(move |py| {
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

            wormhole_tx.send(w.into()).unwrap();
            Ok(())
        })?;

        let wormhole = wormhole_rx.recv().unwrap();

        let instance = Self {
            reactor,
            delegate,
            wormhole,
            code: RefCell::new(None),
            rx,
        };

        Ok(Arc::new(instance))
    }

    pub fn allocate_code(&self) -> PyResult<()> {
        let w = self.wormhole.clone();
        self.reactor.call_from_thread(move |py| {
            w.call_method0(py, "allocate_code")?;
            Ok(())
        })
    }

    pub fn get_code(&self) -> String {
        let code = self.code.borrow().clone();
        if let Some(code) = code {
            return code.clone();
        }

        loop {
            let res = self.rx.recv();
            if let Ok(msg) = res {
                if let WormholeMessage::Code(code) = msg {
                    self.code.replace(Some(code.clone()));
                    return code.clone();
                }
            }
        }
    }

    pub fn wait_open(&self) {
        loop {
            let res = self.rx.recv();
            if let Ok(msg) = res {
                if let WormholeMessage::Versions = msg {
                    return;
                }
            }
        }
    }

    pub fn send_text_message(&self, message: &str) -> PyResult<()> {
        log::debug!("Sending message: {}", message);
        let message = message.to_string();
        let w = self.wormhole.clone();
        self.reactor.call_from_thread(move |py| {
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
        self.reactor.call_from_thread(move |py| {
            w.call_method0(py, "close")?;

            Ok(())
        })
    }
}
