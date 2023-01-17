import Foundation
import AppKit
import SwiftRs

@_cdecl("get_file_thumbnail_base64")
public func getFileThumbnailBase64(path: SRString) -> SRString {
	let path = path.to_string();
	
	let image = NSWorkspace.shared.icon(forFile: path)
	let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!.representation(using: .png, properties: [:])!

	return SRString(bitmap.base64EncodedString())
}

class Volume: NSObject {
	internal init(
		name: String,
		path: String,
		is_root_filesystem: Bool,
		total_capacity: Int,
		available_capacity: Int,
		is_removable: Bool
	) {
		self.name = SRString(name)
		self.is_root_filesystem = is_root_filesystem
		self.mount_point = SRString(path)
		self.total_capacity = total_capacity
		self.available_capacity = available_capacity
		self.is_removable = is_removable
	}
	
	var name: SRString
	var is_root_filesystem: Bool
	var mount_point: SRString
	var total_capacity: Int
	var available_capacity: Int
	var is_removable: Bool
}

@_cdecl("get_mounts")
public func getMounts() -> SRObjectArray {
	   let keys: [URLResourceKey] = [
		.volumeNameKey,
		.volumeIsRootFileSystemKey,
		.canonicalPathKey,
		.volumeTotalCapacityKey,
		.volumeAvailableCapacityKey,
		.volumeIsRemovableKey,
		.volumeIsEjectableKey,
	]
	let paths = FileManager().mountedVolumeURLs(includingResourceValuesForKeys: keys, options: [])
	
	var validMounts: [Volume] = []
	
	guard let urls = paths else {
		return SRObjectArray(validMounts)
	}

	for url in urls {
		let components = url.pathComponents
		if components.count == 1 || components.count > 1
			&& components[1] == "Volumes"
		{
			let metadata = try? url.promisedItemResourceValues(forKeys: Set(keys))
			
			let volume = Volume(
				name: metadata?.volumeName ?? url.absoluteString,
				path: url.absoluteString,
				is_root_filesystem: metadata?.volumeIsRootFileSystem ?? false,
				total_capacity: metadata?.volumeTotalCapacity ?? 0,
				available_capacity: metadata?.volumeAvailableCapacity ?? 0,
				is_removable: (metadata?.volumeIsRemovable ?? false) || (metadata?.volumeIsEjectable ?? false)
			)

			validMounts.append(volume)
		}
	}
	
	return SRObjectArray(validMounts)
}
