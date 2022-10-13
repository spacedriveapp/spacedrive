/// Object Kind
///
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, IntEnum)]
#[repr(u8)]
pub enum ObjectKind {
	// A file that can not be identified by the indexer
	Unknown = 0,
	// A known filetype, but without specific support
	Document = 1,
	// A virtual filesystem directory
	Folder = 2,
	// A file that contains human-readable text
	Text = 3,
	// A virtual directory int
	Package = 4,
	// An image file
	Image = 5,
	// An audio file
	Audio = 6,
	// A video file
	Video = 7,
	// A compressed archive of data
	Archive = 8,
	// An executable, program or application
	Executable = 9,
	// A link to another object
	Alias = 10,
	// Raw bytes encrypted by Spacedrive with self contained metadata
	Encrypted = 11,
	// A link can open web pages, apps or Spaces
	Key = 12,
	// A link can open web pages, apps or Spaces
	Link = 13,
	// A special filetype that represents a preserved webpage
	WebPageArchive = 14,
	// A widget is a mini app that can be placed in a Space at various sizes, associated Widget struct required
	Widget = 15,
	// Albums can only have one level of children, and are associated with the Album struct
	Album = 16,
	// Its like a folder, but appears like a stack of files, designed for burst photos / associated groups of files
	Collection = 17,
	// You know, text init
	Font = 18,
	// 3D Object
	Mesh = 19,
	// Editable source code file
	Code = 20,
	// Database file
	Database = 21,
}
