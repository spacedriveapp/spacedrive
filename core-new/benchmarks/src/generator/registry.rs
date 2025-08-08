use super::{DatasetGenerator, FileSystemGenerator};

pub fn registered_generators() -> Vec<Box<dyn DatasetGenerator>> {
    vec![Box::new(FileSystemGenerator::default())]
}
