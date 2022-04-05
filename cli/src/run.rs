use entity::namespace;
use entity::user;
use migration::sea_orm::ActiveValue::Set;
use migration::sea_orm::ColumnTrait;
use migration::sea_orm::QueryFilter;
use migration::sea_orm::{DatabaseConnection, EntityTrait};
use migration::{sea_orm::Database, Migrator, MigratorTrait};
use rand::{distributions::Alphanumeric, Rng};
use session::Session;
use std::path::PathBuf;
use workers::app::App;

static USER_NAME: &str = "cli-user";

async fn query_default_user(conn: &DatabaseConnection) -> Option<user::Model> {
    user::Entity::find()
        .filter(user::Column::Name.eq(USER_NAME))
        .one(conn)
        .await
        .expect("Failed to check if cli user exists in database")
}

async fn get_or_create_default_user(conn: &DatabaseConnection) -> user::Model {
    if let Some(default_user) = query_default_user(conn).await {
        return default_user;
    }

    let client_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let client_secret: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let to_be_inserted = user::ActiveModel {
        name: Set(USER_NAME.into()),
        client_id: Set(client_id),
        client_secret: Set(client_secret),
        created_at: Set(chrono::DateTime::into(chrono::Utc::now())),
        ..Default::default()
    };
    let insert_res = user::Entity::insert(to_be_inserted)
        .exec(conn)
        .await
        .unwrap();

    // Create a namespace for the user
    let to_be_inserted = namespace::ActiveModel {
        name: Set("default".into()),
        user_id: Set(insert_res.last_insert_id),
        created_at: Set(chrono::DateTime::into(chrono::Utc::now())),
        ..Default::default()
    };
    namespace::Entity::insert(to_be_inserted)
        .exec(conn)
        .await
        .unwrap();

    return query_default_user(conn).await.unwrap();
}

pub async fn start(path_buf: PathBuf) {
    let conn = Database::connect(
        std::env::var("DATABASE_URL")
            .expect("No DATABASE_URL environment variable found.")
            .as_str(),
    )
    .await
    .expect("Database connection failed");

    Migrator::up(&conn, None).await.unwrap();

    let user = get_or_create_default_user(&conn).await;
    let session = Session {
        user_id: user.id,
        conn,
    };

    let app = App::new(
        session,
        "default".into(),
        path_buf,
        "main.js".into(),
        "cli-deployment".into(),
    );

    workers::run(Some(app)).await;
}
