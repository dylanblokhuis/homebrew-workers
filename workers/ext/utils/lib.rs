use deno_core::{include_js_files, Extension};

pub fn init() -> Extension {
    Extension::builder()
        .js(include_js_files!(
            prefix "ext/utils",
            "01_utils.js",
        ))
        .build()
}

// #[op]
// async fn op_kv_set(
//     state: Rc<RefCell<OpState>>,
//     name: String,
//     value: String,
// ) -> Result<(), AnyError> {
//     println!("received op! {} {}", name, value);

//     Ok(())
// }
