pub fn log(context: &str, detail: &str) {
    tuikit::log::log(crate::APP, context, detail);
}

pub fn path_hint() -> String {
    tuikit::log::path_hint(crate::APP)
}
