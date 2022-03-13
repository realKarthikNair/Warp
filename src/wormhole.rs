use std::sync::{Arc, Weak, Mutex, mpsc};
use std::cell::RefCell;
use std::sync::mpsc::{Receiver, Sender};
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;

enum WormholeMessage {
    Code(String),
    Message(Vec<u8>),
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
    tx: Sender<WormholeMessage>
}

impl WormholeDelegate {
    fn new(tx: Sender<WormholeMessage>) -> Self {
        Self {
            tx
        }
    }
}

#[pymethods]
impl WormholeDelegate {
    fn wormhole_got_code(&self, code: &str) {
        log::debug!("Code: {}", code);
        self.tx.send(WormholeMessage::code(code));
    }

    fn wormhole_got_message(&self, msg: &[u8]) {
        log::debug!("Data: {} bytes", msg.len());
        self.tx.send(WormholeMessage::msg(msg));
    }

    fn wormhole_close(&self) {
        log::debug!("Wormhole closed");
        self.tx.send(WormholeMessage::Close);
    }
}

pub struct Wormhole {
    delegate: PyObject,
    reactor: PyObject,
    rx: Receiver<WormholeMessage>,
}

impl Wormhole {
    pub fn new() -> PyResult<Arc<Wormhole>> {
        let (tx, rx) = mpsc::channel();

        let res: PyResult<(PyObject, PyObject)> = Python::with_gil(|py| {
            let epollreactor = py.import("twisted.internet.epollreactor")?;
            epollreactor.getattr("install")?.call0()?;

            let reactor: PyObject = py.import("twisted.internet.reactor")?.into();

            let delegate: PyObject = WormholeDelegate::new(tx).into_py(py);
            Ok((reactor, delegate))
        });

        let (reactor, delegate) = res?;

        let mut instance = Self {
            delegate,
            reactor,
            rx,
        };

        Ok(Arc::new(instance))
    }

    pub fn send_file(&self, filename: &str) -> PyResult<()> {
        Ok(())
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
