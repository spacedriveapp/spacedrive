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