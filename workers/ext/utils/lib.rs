use deno_core::{include_js_files, Extension};

pub fn init() -> Extension {
    Extension::builder()
        .js(include_js_files!(
            prefix "ext/utils",
            "01_utils.js",
        ))
        .build()
}
