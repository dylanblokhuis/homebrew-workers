use std::{cell::RefCell, rc::Rc};

use deno_core::{error::AnyError, include_js_files, op, Extension, OpState};
use entity::namespace;
use entity::store;
use migration::sea_orm::ActiveValue::Set;
use migration::sea_orm::ColumnTrait;
use migration::sea_orm::EntityTrait;
use migration::sea_orm::QueryFilter;
use session::Session;

pub fn init(maybe_session: Option<Session>) -> Extension {
    Extension::builder()
        .js(include_js_files!(
            prefix "ext/kv",
            "01_kv.js",
        ))
        .ops(vec![op_kv_set::decl(), op_kv_get::decl()])
        .state(move |state| {
            if let Some(session) = maybe_session.clone() {
                state.put::<Session>(session);
            }
            Ok(())
        })
        .build()
}

#[op]
async fn op_kv_set(
    state: Rc<RefCell<OpState>>,
    key: String,
    value: String,
) -> Result<(), AnyError> {
    let namespace_name = "default";
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = namespace::Entity::find()
        .filter(namespace::Column::Name.eq(namespace_name))
        .filter(namespace::Column::UserId.eq(session.user_id))
        .one(&session.conn)
        .await?
        .expect("This user has no default namespace, something is going wrong here..");

    let maybe_item = store::Entity::find()
        .filter(store::Column::Key.eq(key.clone()))
        .filter(store::Column::NamespaceId.eq(namespace.id))
        .one(&session.conn)
        .await?;

    if let Some(item) = maybe_item {
        let mut to_be_updated: store::ActiveModel = item.into();
        to_be_updated.value = Set(value);

        store::Entity::update(to_be_updated)
            .exec(&session.conn)
            .await?;

        return Ok(());
    }

    let to_be_inserted = store::ActiveModel {
        key: Set(key),
        value: Set(value),
        created_at: Set(chrono::DateTime::into(chrono::Utc::now())),
        namespace_id: Set(namespace.id),
        ..Default::default()
    };

    store::Entity::insert(to_be_inserted)
        .exec(&session.conn)
        .await?;

    Ok(())
}

#[op]
async fn op_kv_get(state: Rc<RefCell<OpState>>, key: String) -> Result<Option<String>, AnyError> {
    let namespace_name = "default";
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = namespace::Entity::find()
        .filter(namespace::Column::Name.eq(namespace_name))
        .filter(namespace::Column::UserId.eq(session.user_id))
        .one(&session.conn)
        .await?
        .expect("This user has no default namespace, something is going wrong here..");

    let store_item = store::Entity::find()
        .filter(store::Column::Key.eq(key))
        .filter(store::Column::NamespaceId.eq(namespace.id))
        .one(&session.conn)
        .await?;

    if let Some(item) = store_item {
        return Ok(Some(item.value));
    }

    Ok(None)
}
