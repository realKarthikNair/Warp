use crate::glib::clone;
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex, Weak};
use std::thread;
use std::thread::JoinHandle;

use crate::globals;

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
}

#[pymethods]
impl WormholeDelegate {
    fn wormhole_got_code(&self, code: &str) {
        log::debug!("Code: {}", code);
        self.tx.send(WormholeMessage::code(code));
    }

    fn wormhole_got_unverified_key(&self, key: &[u8]) {
        log::debug!("Got unverified key {:?}", key);
    }

    fn wormhole_got_verifier(&self, verifier: &[u8]) {
        log::debug!("Got verifier {:?}", verifier);
    }

    fn wormhole_got_versions(&self, versions: &PyDict) {
        log::debug!("Got versions {}", versions);
        self.tx.send(WormholeMessage::Versions);
    }

    fn wormhole_got_message(&self, msg: &[u8]) {
        log::debug!("Data: {} bytes", msg.len());
        self.tx.send(WormholeMessage::msg(msg));
    }

    fn wormhole_closed(&self, result: PyObject) {
        log::debug!("Wormhole closed: {:?}", result);
        self.tx.send(WormholeMessage::Close);
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
        let cloned_reactor = reactor.reactor.clone();
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

            wormhole_tx.send(w.into());
            Ok(())
        });

        let wormhole = wormhole_rx.recv().unwrap();

        let mut instance = Self {
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
        let res = self.reactor.call_from_thread(move |py| {
            let json = py.import("json")?;
            let dict = PyDict::new(py);
            let offer = PyDict::new(py);
            offer.set_item("message", message.to_string());
            dict.set_item("offer", offer);

            log::debug!("{:?}", dict.to_string());
            let bin_dict = json
                .call_method1("dumps", (dict,))?
                .call_method1("encode", ("utf-8",))?;
            let res = w.call_method1(py, "send_message", (bin_dict,));
            if let Err(err) = res {
                err.print(py);
            }
            Ok(())
        });

        Ok(())
    }

    pub fn close(&self) -> PyResult<()> {
        let w = self.wormhole.clone();
        self.reactor.call_from_thread(move |py| {
            w.call_method0(py, "close")?;

            Ok(())
        })
    }

    fn wormhole_got_code(&self, code: &str) {
        log::debug!("Code: {}", code)
    }

    fn wormhole_got_message(&self, code: &[u8]) {
        log::debug!("Got data: {} bytes", code.len());
    }

    fn wormhole_close(&self, result: &str) {
        log::debug!("Wormhole closed");
    }
}

#[pyclass]
pub struct PythonClosure {
    func: RefCell<Option<Box<dyn for<'py> FnOnce(Python<'py>) -> PyResult<()> + Send>>>,
}

impl PythonClosure {
    pub fn new<F: 'static>(func: F) -> Self
    where
        F: for<'py> FnOnce(Python<'py>) -> PyResult<()> + Send,
    {
        Self {
            func: RefCell::new(Some(Box::new(func))),
        }
    }

    pub fn into_closure(self, py: Python) -> PyResult<PyObject> {
        self.into_py(py).getattr(py, "call")
    }
}

#[pymethods]
impl PythonClosure {
    pub fn call(&self, py: Python) -> PyResult<()> {
        let func = self.func.take();
        if let Some(func) = func {
            let res = func(py);
            if let Err(err) = &res {
                log::debug!("Error from python closure");
                err.print(py);
            }

            return res;
        } else {
            log::error!("MethodHelper::run() called more than once");
        }

        Ok(())
    }
}

#[pyclass]
pub struct TwistedReactor {
    reactor: PyObject,
    thread_handle: JoinHandle<PyResult<()>>,
}

impl TwistedReactor {
    pub fn new() -> PyResult<TwistedReactor> {
        log::debug!("Creating reactor");
        let (tx, rx) = mpsc::channel();
        let thread_handle: JoinHandle<PyResult<()>> = thread::spawn(clone!(@strong tx => move || {
            // We must call this on the twisted thread
            pyo3::prepare_freethreaded_python();
            let res: PyResult<()> = Python::with_gil(|py| {
                log::debug!("Installing reactor");
                let epollreactor = py.import("twisted.internet.epollreactor")?;
                epollreactor.getattr("install")?.call0()?;

                let reactor: PyObject = py.import("twisted.internet.reactor")?.into();
                tx.send(reactor.clone()).unwrap();
                reactor.getattr(py, "run")?.call0(py)?;

                Ok(())
            });

            res
        }));

        let reactor = rx.recv().unwrap();
        log::debug!("Got reactor");

        Ok(Self {
            reactor,
            thread_handle,
        })
    }
    fn call_from_thread<F: 'static>(&self, func: F) -> PyResult<()>
    where
        F: for<'py> FnOnce(Python<'py>) -> PyResult<()> + Send,
    {
        let closure = PythonClosure::new(func);
        let res: PyResult<()> = Python::with_gil(|py| {
            self.reactor
                .call_method1(py, "callFromThread", (closure.into_closure(py)?,))?;
            Ok(())
        });

        Ok(())
    }
}
