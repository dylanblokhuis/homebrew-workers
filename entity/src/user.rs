use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::namespace;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub latest_deployment: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Namespaces,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Namespaces => Entity::has_many(namespace::Entity).into(),
        }
    }
}

impl Related<super::namespace::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Namespaces.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
