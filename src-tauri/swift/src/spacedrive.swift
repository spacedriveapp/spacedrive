import Foundation
import AppKit

public class FFIString: NSObject {
    var data: UnsafePointer<CChar>
    var length: UInt64
    
    init(data: UnsafePointer<CChar>, length: UInt64) {
        self.data = data
        self.length = length
    }
}

public class FFIData: NSObject {
    var data: UnsafePointer<UInt8>
    var length: UInt64
    
    init(data: UnsafePointer<UInt8>, length: UInt64) {
        self.data = data
        self.length = length
    }
}

@_cdecl("get_file_thumbnail")
public func getFileThumbnail(path_ptr: UnsafePointer<CChar>, path_length: UInt64) -> FFIData {
    print(path_length)
    let path = String(data: Data(bytes: path_ptr, count: Int(path_length)), encoding: String.Encoding.utf8)!
    print(path)
    let image = NSWorkspace.shared.icon(forFile: path)
    let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!.representation(using: .png, properties: [:])!
    
    let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bitmap.count)
    bitmap.copyBytes(to: pointer, count: bitmap.count)
    
    return FFIData(data: UnsafePointer(pointer), length: UInt64(bitmap.count))
}

@_cdecl("test")
public func test(path_ptr: UnsafePointer<CChar>, path_length: UInt64) -> FFIString {
    let path = String(data: Data(bytes: path_ptr, count: Int(path_length)), encoding: String.Encoding.utf8)!
    print(path)
    let image = NSWorkspace.shared.icon(forFile: path)
    let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!.representation(using: .png, properties: [:])!
    
    let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bitmap.count)
    bitmap.copyBytes(to: pointer, count: bitmap.count)
   
    let data = UnsafePointer<UInt8>(pointer);
    let length = UInt64(bitmap.count)
    
    print(data)
    print(length)
    
    let ret = FFIString(
        data: UnsafePointer<CChar>(strdup(path)!),
        length: UInt64(path.lengthOfBytes(using: .utf8))
    )
    
    print(ret)
    
    return ret
}
