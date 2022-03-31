use migration::sea_orm::DatabaseConnection;

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: i32,
    pub conn: DatabaseConnection,
}
