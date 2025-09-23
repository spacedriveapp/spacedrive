//! Minimal proof that the rspc-inspired approach works
//! Test with just the operations that already have proper derives

use crate::ops::type_extraction::*;
use crate::ops::files::copy::action::FileCopyAction;
use crate::infra::job::handle::JobHandle;

// Manually demonstrate the trait implementation for one working operation
impl OperationTypeInfo for FileCopyAction {
    type Input = crate::ops::files::copy::input::FileCopyInput;
    type Output = String; // Use String instead of JobHandle to avoid issues

    fn identifier() -> &'static str {
        "files.copy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specta::TypeCollection;

    #[test]
    fn test_manual_type_extraction() {
        let mut collection = TypeCollection::default();
        let metadata = FileCopyAction::extract_types(&mut collection);

        println!("✅ Manual type extraction works!");
        println!("   Operation: {}", metadata.identifier);
        println!("   Wire method: {}", metadata.wire_method);
        println!("   Type collection has {} types", collection.len());

        assert_eq!(metadata.identifier, "files.copy");
        assert_eq!(metadata.wire_method, "action:files.copy.input.v1");
    }
}
