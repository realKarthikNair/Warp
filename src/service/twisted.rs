use crate::glib::clone;
use crate::service::util::PythonClosure;
use pyo3::prelude::*;
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;

#[pyclass]
pub struct TwistedReactor {
    py_reactor: PyObject,
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

        let py_reactor = rx.recv().unwrap();
        log::debug!("Got reactor");

        Ok(Self {
            py_reactor,
            thread_handle,
        })
    }

    pub fn call_from_thread<F: 'static>(&self, func: F) -> PyResult<()>
    where
        F: for<'py> FnOnce(Python<'py>) -> PyResult<()> + Send,
    {
        let closure = PythonClosure::new(func);
        let res: PyResult<()> = Python::with_gil(|py| {
            self.py_reactor()
                .call_method1(py, "callFromThread", (closure.into_closure(py)?,))?;
            Ok(())
        });
        res?;

        Ok(())
    }

    pub fn py_reactor(&self) -> &PyObject {
        &self.py_reactor
    }
}
