use sea_orm::{Database, DbConn, DbErr};

pub async fn connect() -> Result<DbConn, DbErr> {
    let url = "postgres://memo_mrfj_user:lo4eJLOmcgBEgsI5LbTwh4ju5OJWb5Vr@dpg-d396progjchc73dh5mr0-a.oregon-postgres.render.com/memo"; // replace with your real credentials
    Database::connect(url).await
}
