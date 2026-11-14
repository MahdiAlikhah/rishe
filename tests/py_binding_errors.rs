#![cfg(feature = "python")]

// Needs the `python` feature; the CI `features` job runs it with python3-dev.

use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::*;
use rishe::Pipeline;

#[test]
fn number_to_words_reports_an_out_of_range_int() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let pipeline =
            Pipeline::py_new(true, true, true, None, None, None, None).expect("pipeline");
        let value = py.eval_bound("10**25", None, None).expect("big int");
        let error = pipeline.py_number_to_words(&value).unwrap_err();
        assert!(
            error.is_instance_of::<PyOverflowError>(py),
            "{error:?} is not OverflowError"
        );
    });
}
