use std::{cell::RefCell, rc::Rc};

use deno_core::{error::AnyError, include_js_files, op, Extension, OpState};

pub fn init() -> Extension {
    Extension::builder()
        .js(include_js_files!(
            prefix "ext/kv",
            "01_kv.js",
        ))
        .ops(vec![op_kv_set::decl()])
        .build()
}

#[op]
async fn op_kv_set(
    state: Rc<RefCell<OpState>>,
    name: String,
    value: String,
) -> Result<(), AnyError> {
    println!("received op! {} {}", name, value);

    Ok(())
}
