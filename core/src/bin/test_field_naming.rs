//! Test field naming conversion in Specta

use specta::{Type, TypeCollection};
use specta_swift::Swift;

#[derive(Type)]
struct TestStruct {
	snake_case_field: String,
	another_field_name: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("🧪 Testing field naming conversion...");

	let types = TypeCollection::default().register::<TestStruct>();

	let swift = Swift::new().naming(specta_swift::NamingConvention::PascalCase);

	let output = swift.export(&types)?;
	println!("📄 Generated Swift:\n{}", output);

	// Check if snake_case_field becomes snakeCaseField
	if output.contains("snakeCaseField") {
		println!("✅ Field naming conversion is working");
	} else {
		println!("❌ Field naming conversion is NOT working");
	}

	Ok(())
}
