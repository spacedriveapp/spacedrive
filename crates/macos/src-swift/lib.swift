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

class Volume: NSObject {
    var name: SRString
    var path: SRString
    var total_capacity: Int
    var available_capacity: Int
    var is_removable: Bool
    var is_ejectable: Bool
    var is_root_filesystem: Bool

    internal init(name: String, path: String, total_capacity: Int, available_capacity: Int, is_removable: Bool, is_ejectable: Bool, is_root_filesystem: Bool) {
        self.name = SRString(name)
        self.path = SRString(path)
        self.total_capacity = total_capacity
        self.available_capacity = available_capacity
        self.is_removable = is_removable
        self.is_ejectable = is_ejectable
        self.is_root_filesystem = is_root_filesystem
    }
}

@_cdecl("get_mounts")
public func getMounts() -> SRObjectArray {
       let keys: [URLResourceKey] = [
        .volumeNameKey,
        .volumeIsRemovableKey,
        .volumeIsEjectableKey,
        .volumeTotalCapacityKey,
        .volumeAvailableCapacityKey,
        .volumeIsRootFileSystemKey,
    ]
    let paths = FileManager().mountedVolumeURLs(includingResourceValuesForKeys: keys, options: [])
    
    var validMounts: [Volume] = []
    
    if let urls = paths {
        for url in urls {
            let components = url.pathComponents
            if components.count == 1 || components.count > 1
               && components[1] == "Volumes"
            {
                let metadata = try? url.promisedItemResourceValues(forKeys: Set(keys))
                
                let volume = Volume(
                    name: metadata?.volumeName ?? "",
                    path: url.path,
                    total_capacity: metadata?.volumeTotalCapacity ?? 0,
                    available_capacity: metadata?.volumeAvailableCapacity ?? 0,
                    is_removable: metadata?.volumeIsRemovable ?? false,
                    is_ejectable: metadata?.volumeIsEjectable ?? false,
                    is_root_filesystem: metadata?.volumeIsRootFileSystem ?? false
                )
                                
                validMounts.append(volume)
            }
        }
    }
    
    return SRObjectArray(validMounts)
}
