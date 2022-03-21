pub use sea_schema::migration::*;

mod m20220101_000001_create_store_table;
mod m20220321_122000_create_users_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_store_table::Migration),
            Box::new(m20220321_122000_create_users_table::Migration),
        ]
    }
}
