//! Migration to add hardware specifications to devices table
//!
//! Extends the devices table with comprehensive hardware information including
//! CPU specs, memory, form factor, manufacturer, GPU, and storage details.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
	async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		// Phase 1: Core Hardware Specifications

		// CPU model name
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::CpuModel).string())
					.to_owned(),
			)
			.await?;

		// CPU architecture
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::CpuArchitecture).string())
					.to_owned(),
			)
			.await?;

		// CPU physical cores
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::CpuCoresPhysical).unsigned())
					.to_owned(),
			)
			.await?;

		// CPU logical cores
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::CpuCoresLogical).unsigned())
					.to_owned(),
			)
			.await?;

		// CPU frequency in MHz
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::CpuFrequencyMhz).big_integer())
					.to_owned(),
			)
			.await?;

		// Total memory in bytes
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::MemoryTotalBytes).big_integer())
					.to_owned(),
			)
			.await?;

		// Form factor
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::FormFactor).string())
					.to_owned(),
			)
			.await?;

		// Manufacturer
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::Manufacturer).string())
					.to_owned(),
			)
			.await?;

		// Phase 2: Extended Hardware

		// GPU models (JSON array of strings)
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::GpuModels).json())
					.to_owned(),
			)
			.await?;

		// Boot disk type
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::BootDiskType).string())
					.to_owned(),
			)
			.await?;

		// Boot disk capacity in bytes
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::BootDiskCapacityBytes).big_integer())
					.to_owned(),
			)
			.await?;

		// Total swap in bytes
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.add_column(ColumnDef::new(Devices::SwapTotalBytes).big_integer())
					.to_owned(),
			)
			.await?;

		Ok(())
	}

	async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::CpuModel)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::CpuArchitecture)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::CpuCoresPhysical)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::CpuCoresLogical)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::CpuFrequencyMhz)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::MemoryTotalBytes)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::FormFactor)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::Manufacturer)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::GpuModels)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::BootDiskType)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::BootDiskCapacityBytes)
					.to_owned(),
			)
			.await?;

		manager
			.alter_table(
				Table::alter()
					.table(Devices::Table)
					.drop_column(Devices::SwapTotalBytes)
					.to_owned(),
			)
			.await?;

		Ok(())
	}
}

#[derive(DeriveIden)]
enum Devices {
	Table,
	CpuModel,
	CpuArchitecture,
	CpuCoresPhysical,
	CpuCoresLogical,
	CpuFrequencyMhz,
	MemoryTotalBytes,
	FormFactor,
	Manufacturer,
	GpuModels,
	BootDiskType,
	BootDiskCapacityBytes,
	SwapTotalBytes,
}
