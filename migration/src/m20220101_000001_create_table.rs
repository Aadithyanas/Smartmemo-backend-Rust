use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // users table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("users"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("username")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("email"))
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Alias::new("password")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // voice_memos1 table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("voice_memos1"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("title")).string().not_null())
                    .col(ColumnDef::new(Alias::new("audio_blob")).blob())
                    .col(ColumnDef::new(Alias::new("transcript")).text().null())
                    .col(ColumnDef::new(Alias::new("translate")).text().null())
                    .col(ColumnDef::new(Alias::new("summary")).text().null())
                    .col(ColumnDef::new(Alias::new("tags")).text().null())
                    .col(ColumnDef::new(Alias::new("duration")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("voice_memos1"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // helperApp table
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("helperApp"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("gemini_key")).string())
                    .col(ColumnDef::new(Alias::new("elevenlabs_key")).string())
                    .col(ColumnDef::new(Alias::new("user_id")).uuid().not_null())
                    .col(ColumnDef::new(Alias::new("action")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("timestamp"))
                            .timestamp()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Alias::new("helperApp"), Alias::new("user_id"))
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("helperApp")).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("voice_memos1")).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Alias::new("users")).to_owned())
            .await?;
        Ok(())
    }
}
