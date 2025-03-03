use sea_orm_migration::{MigrationTrait, MigratorTrait};

mod m20240626_0001_init;
mod m20240809_0001_change_node_id_i64;
mod m20240824_0001_add_room_destroy_and_record;
mod m20240929_0001_add_multi_tenancy;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240626_0001_init::Migration),
            Box::new(m20240809_0001_change_node_id_i64::Migration),
            Box::new(m20240824_0001_add_room_destroy_and_record::Migration),
            Box::new(m20240929_0001_add_multi_tenancy::Migration),
        ]
    }
}
