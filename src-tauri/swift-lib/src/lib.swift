import Foundation
import AppKit

// Size: 24 bytes
public class SRArray<T>: NSObject {
    var pointer: UnsafePointer<T>
    var length: Int
    
    init(_ data: [T]) {
        let mut_data = UnsafeMutablePointer<T>.allocate(capacity: data.count)
        mut_data.initialize(from: data, count: data.count)
        
        self.pointer = UnsafePointer(mut_data)
        self.length = data.count
    }
}

// Size: 16 bytes
public class SRData: NSObject {
    var data: SRArray<UInt8>

    init(_ data: [UInt8]) {
        self.data = SRArray(data)
    }
    
    func to_data() -> Data {
        return Data(bytes: self.data.pointer, count: self.data.length)
    }
}

// Size: 16 bytes
public class SRString: SRData {
    init(_ string: String) {
        super.init(Array(string.utf8))
    }

    func to_string() -> String {
        return String(bytes: self.to_data(), encoding: .utf8)!
    }
}

@_cdecl("return_data")
public func returnData() -> SRData {
    return SRData([1,2,3])
}


@_cdecl("return_string")
public func returnString() -> SRString {
    return SRString("123456")
}

@_cdecl("echo_string")
public func echoString(string: SRString) {
    print(string.to_string())
}

// SRstring pointer is passed to rust correctly
// data pointer is passed to rust correctly
// guessing that the type of SRArray isn't the same
@_cdecl("allocate_string")
public func allocate_string(data: UnsafePointer<UInt8>, size: Int) -> SRString {
    let buffer = UnsafeBufferPointer(start: data, count: size)
    let string = String(bytes: buffer, encoding: .utf8)!;
    let SRstring = SRString(string);
    return SRstring
}

@_cdecl("get_file_thumbnail_base64")
public func getFileThumbnailBase64(path: SRString) -> SRString {
    let path = path.to_string();
    
    let image = NSWorkspace.shared.icon(forFile: path)
    let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!)!.representation(using: .png, properties: [:])!

    return SRString(bitmap.base64EncodedString())
}

