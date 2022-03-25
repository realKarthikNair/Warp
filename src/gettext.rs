use gettextrs::*;
use std::fmt::Display;

pub mod duration;

pub fn gettextf(format: &str, args: &[&dyn Display]) -> String {
    let mut s = gettext(format);

    for arg in args {
        s = s.replacen("{}", &arg.to_string(), 1)
    }
    s
}

pub fn ngettextf(msgid: &str, msgid_plural: &str, n: u32, args: &[&dyn Display]) -> String {
    let mut s = ngettext(msgid, msgid_plural, n);

    for arg in args {
        s = s.replacen("{}", &arg.to_string(), 1)
    }
    s
}

pub fn ngettextf_(msgid: &str, msgid_plural: &str, n: u32) -> String {
    ngettextf(msgid, msgid_plural, n, &[&n])
}
