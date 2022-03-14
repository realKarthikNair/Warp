use pyo3::prelude::*;
use std::cell::RefCell;

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
