use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "store")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub key: String,
    #[sea_orm(column_type = "Custom(\"jsonb\".to_owned())")]
    pub value: String,
    pub namespace_id: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Namespace,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Namespace => Entity::belongs_to(super::namespace::Entity)
                .from(Column::NamespaceId)
                .to(super::namespace::Column::Id)
                .into(),
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::namespace::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Namespace.def()
    }
}
