use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

use deno_core::anyhow::Context;
use deno_core::{anyhow::Result, include_js_files, op, Extension, OpState};
use entity::namespace;
use entity::store;
use migration::sea_orm::ActiveValue::Set;
use migration::sea_orm::ColumnTrait;
use migration::sea_orm::EntityTrait;
use migration::sea_orm::QueryFilter;
use migration::DbErr;
use session::Session;

pub fn init(maybe_session: Option<Session>) -> Extension {
    Extension::builder()
        .js(include_js_files!(
            prefix "ext/kv",
            "01_kv.js",
        ))
        .ops(vec![
            op_kv_set::decl(),
            op_kv_get::decl(),
            op_kv_delete::decl(),
            op_kv_clear::decl(),
            op_kv_all::decl(),
        ])
        .state(move |state| {
            if let Some(session) = maybe_session.clone() {
                state.put::<Session>(session);
            }
            Ok(())
        })
        .build()
}

#[op]
async fn op_kv_set(state: Rc<RefCell<OpState>>, key: String, value: String) -> Result<()> {
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = get_namespace(&session).await?;
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
async fn op_kv_get(state: Rc<RefCell<OpState>>, key: String) -> Result<Option<String>> {
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = get_namespace(&session).await?;
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

#[op]
async fn op_kv_delete(state: Rc<RefCell<OpState>>, key: String) -> Result<()> {
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = get_namespace(&session).await?;

    let maybe_item = store::Entity::find()
        .filter(store::Column::Key.eq(key))
        .filter(store::Column::NamespaceId.eq(namespace.id))
        .one(&session.conn)
        .await?;

    if let Some(item) = maybe_item {
        let to_delete = store::ActiveModel {
            id: Set(item.id),
            ..Default::default()
        };

        store::Entity::delete(to_delete).exec(&session.conn).await?;
    }

    Ok(())
}

#[op]
async fn op_kv_clear(state: Rc<RefCell<OpState>>) -> Result<()> {
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = get_namespace(&session).await?;
    store::Entity::delete_many()
        .filter(store::Column::NamespaceId.eq(namespace.id))
        .exec(&session.conn)
        .await?;

    Ok(())
}

#[op]
async fn op_kv_all(state: Rc<RefCell<OpState>>) -> Result<HashMap<String, String>> {
    let session = {
        let state = state.borrow();
        state.borrow::<Session>().clone()
    };

    let namespace = get_namespace(&session).await?;
    let items = store::Entity::find()
        .filter(store::Column::NamespaceId.eq(namespace.id))
        .all(&session.conn)
        .await
        .context("Failed to get store items from database")?;

    let mut map: HashMap<String, String> = HashMap::new();
    for item in items {
        map.insert(item.key, item.value);
    }

    Ok(map)
}

async fn get_namespace(session: &Session) -> Result<namespace::Model, DbErr> {
    let name = "default";

    let namespace = namespace::Entity::find()
        .filter(namespace::Column::Name.eq(name))
        .filter(namespace::Column::UserId.eq(session.user_id))
        .one(&session.conn)
        .await?
        .expect("This user has no default namespace, something is going wrong here..");

    Ok(namespace)
}
