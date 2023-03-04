import Foundation
import AppKit
import SwiftRs

@_cdecl("get_file_thumbnail_base64")
public func getFileThumbnailBase64(path: SRString) -> SRString {
    let path = path.toString();
    
    let image = NSWorkspace.shared.icon(forFile: path)
    let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!.representation(using: .png, properties: [:])!

	return SRString(bitmap.base64EncodedString())
}

// TODO: when SwiftRs gets unfrizzled put this back!
/*
public class Volume: NSObject {
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
		self.total_capacity = UInt64(total_capacity)
		self.available_capacity = UInt64(available_capacity)
		self.is_removable = is_removable
	}
	
	var name: SRString
	var is_root_filesystem: Bool
	var mount_point: SRString
	var total_capacity: UInt64
	var available_capacity: UInt64
	var is_removable: Bool
}
*/

// until SwiftRs is patched for object access we are encoding data as a JSON string
let jsonEncoder = JSONEncoder()

public struct Volume: Codable {
	var name: String
	var is_root_filesystem: Bool
	var mount_point: String
	var total_capacity: Int
	var available_capacity: Int
	var is_removable: Bool
}

@_cdecl("native_get_mounts")
public func getMounts() -> SRString {
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

    if let urls = paths {
        for url in urls {
            let components = url.pathComponents
            if components.count > 1 && components[1] != "Volumes"
            {
                continue
            }

            let metadata = try? url.promisedItemResourceValues(forKeys: Set(keys))
            
            let volume = Volume(
                name: metadata?.volumeName ?? url.absoluteString,
                is_root_filesystem: metadata?.volumeIsRootFileSystem ?? false,
                mount_point: url.path,
                total_capacity: metadata?.volumeTotalCapacity ?? 0,
                available_capacity: metadata?.volumeAvailableCapacity ?? 0,
                is_removable: (metadata?.volumeIsRemovable ?? false) || (metadata?.volumeIsEjectable ?? false)
            )

            validMounts.append(volume)
        }
    }
	
	return SRString(String(data: try! jsonEncoder.encode(validMounts), encoding: .utf8)!)
}
