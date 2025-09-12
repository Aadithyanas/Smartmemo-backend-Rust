use sea_orm::{Database, DbConn, DbErr};

pub async fn connect() -> Result<DbConn, DbErr> {
    let url = "postgres://postgres:mark42@localhost:5432/memo"; // replace with your real credentials
    Database::connect(url).await
}
