// An array of Object kinds.
// Note: The order of this enum should never change, and always be kept in sync with `crates/file-ext/src/kind.rs`
export enum ObjectKindEnum {
	Unknown,
	Document,
	Folder,
	Text,
	Package,
	Image,
	Audio,
	Video,
	Archive,
	Executable,
	Alias,
	Encrypted,
	Key,
	Link,
	WebPageArchive,
	Widget,
	Album,
	Collection,
	Font,
	Mesh,
	Code,
	Database,
	Book,
	Config,
	Dotfile,
	Screenshot,
	Label
}

export type ObjectKindKey = keyof typeof ObjectKindEnum;

// This is ugly, but typescript doesn't support type narrowing for enum index access yet:
// https://github.com/microsoft/TypeScript/issues/38806
export const ObjectKind = ObjectKindEnum as typeof ObjectKindEnum & {
	[key: number]: ObjectKindKey | undefined;
};
