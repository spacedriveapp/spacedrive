//! Volume entity

use crate::volume::types::TrackedVolume;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "volumes")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub uuid: Uuid,
    pub fingerprint: String,
    pub display_name: Option<String>,
    pub tracked_at: DateTimeUtc,
    pub last_seen_at: DateTimeUtc,
    pub is_online: bool,
    pub total_capacity: Option<i64>,
    pub available_capacity: Option<i64>,
    pub read_speed_mbps: Option<i32>,
    pub write_speed_mbps: Option<i32>,
    pub last_speed_test_at: Option<DateTimeUtc>,
    pub file_system: Option<String>,
    pub mount_point: Option<String>,
    pub is_removable: Option<bool>,
    pub is_network_drive: Option<bool>,
    pub device_model: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Convert database model to tracked volume
    pub fn to_tracked_volume(&self) -> TrackedVolume {
        TrackedVolume {
            id: self.id,
            uuid: self.uuid,
            fingerprint: crate::volume::VolumeFingerprint(self.fingerprint.clone()),
            display_name: self.display_name.clone(),
            tracked_at: self.tracked_at,
            last_seen_at: self.last_seen_at,
            is_online: self.is_online,
            total_capacity: self.total_capacity.map(|c| c as u64),
            available_capacity: self.available_capacity.map(|c| c as u64),
            read_speed_mbps: self.read_speed_mbps.map(|s| s as u32),
            write_speed_mbps: self.write_speed_mbps.map(|s| s as u32),
            last_speed_test_at: self.last_speed_test_at,
            file_system: self.file_system.clone(),
            mount_point: self.mount_point.clone(),
            is_removable: self.is_removable,
            is_network_drive: self.is_network_drive,
            device_model: self.device_model.clone(),
        }
    }
}