use anyhow::Result;
use libusb;

pub async fn discover_storage_devices() -> Result<()> {
  let mut context = libusb::Context::new().unwrap();

  for mut device in context.devices().unwrap().iter() {
    let device_desc = device.device_descriptor().unwrap();

    println!(
      "Bus {:03} Device {:03} ID {:04x}:{:04x}",
      device.bus_number(),
      device.address(),
      device_desc.vendor_id(),
      device_desc.product_id()
    );
  }
}
