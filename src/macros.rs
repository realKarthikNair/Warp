macro_rules! cargo_output_path {
    () => {
        concat!(env!("CARGO_OUT_DIR"), "/")
    };
    ($path:literal) => {
        concat!(cargo_output_path!(), $path)
    };
}

macro_rules! include_cargo_output_path_bytes {
    ($path:literal) => {
        include_bytes!(cargo_output_path!($path))
    };
}

#[allow(unused_macros)]
macro_rules! gresource_path {
    () => {
        "/net/felinira/warp/"
    };
    ($path:literal) => {
        concat!(gresource_path!(), $path)
    };
}
