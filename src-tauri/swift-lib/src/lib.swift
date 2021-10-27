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

public struct Volume : Codable {
    var name: String
    var path: String
    var total_capacity: Int
    var available_capacity: Int
    var is_removable: Bool
    var is_ejectable: Bool
    var is_root_filesystem: Bool
}


@_cdecl("get_mounts")
public func getMounts() -> SRString {
       let keys: [URLResourceKey] = [
        .volumeNameKey,
        .volumeIsRemovableKey,
        .volumeIsEjectableKey,
        .volumeTotalCapacityKey,
        .volumeAvailableCapacityKey,
        .volumeIsRootFileSystemKey,
    ]
    let paths = FileManager().mountedVolumeURLs(includingResourceValuesForKeys: keys, options: [])
    
    if let urls = paths {
        var validMounts: [Volume] = []
        
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
        let jsonData = try? JSONEncoder().encode(validMounts)
        
        if jsonData != nil {
            let jsonString = String(data: jsonData!, encoding: .utf8)!
            return SRString(jsonString)
        }
        return SRString("")

    }
    return SRString("")
}



