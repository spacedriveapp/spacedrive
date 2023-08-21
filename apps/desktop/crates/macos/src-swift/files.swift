import AppKit
import SwiftRs

extension NSBitmapImageRep {
    var png: Data? { representation(using: .png, properties: [:]) }
}

extension Data {
    var bitmap: NSBitmapImageRep? { NSBitmapImageRep(data: self) }
}

extension NSImage {
    var png: Data? { tiffRepresentation?.bitmap?.png }
}

class OpenWithApplication: NSObject {
    var name: SRString
    var id: SRString
    var url: SRString
    var icon: SRData
    
    init(name: SRString, id: SRString, url: SRString, icon: SRData) {
        self.name = name
        self.id = id
        self.url = url
        self.icon = icon
    }
}

@_cdecl("get_open_with_applications")
func getOpenWithApplications(urlString: SRString) -> SRObjectArray {
    let url: URL
    if #available(macOS 13.0, *) {
        url = URL(filePath: urlString.toString())
    } else {
        // Fallback on earlier versions
        url = URL(fileURLWithPath: urlString.toString())
    }
    
    let appURLs: [URL]
    if #available(macOS 12.0, *) {
        appURLs = NSWorkspace.shared.urlsForApplications(toOpen: url)
    } else {
        // Fallback for macOS versions prior to 12
        
        // Get type identifier from file URL
        let fileType: String
        if #available(macOS 11.0, *) {
            guard let _fileType = (try? url.resourceValues(forKeys: [.typeIdentifierKey]))?.typeIdentifier
            else {
                print("Failed to fetch file type for the specified file URL")
                return SRObjectArray([])
            }
            
            fileType = _fileType
        } else {
            // Fallback for macOS versions prior to 11
            guard
                let _fileType = UTTypeCreatePreferredIdentifierForTag(
                    kUTTagClassFilenameExtension, url.pathExtension as CFString, nil)?.takeRetainedValue()
            else {
                print("Failed to fetch file type for the specified file URL")
                return SRObjectArray([])
            }
            fileType = _fileType as String
        }
        
        // Locates an array of bundle identifiers for apps capable of handling a specified content type with the specified roles.
        guard
            let bundleIds = LSCopyAllRoleHandlersForContentType(fileType as CFString, LSRolesMask.all)?
                .takeRetainedValue() as? [String]
        else {
            print("Failed to fetch bundle IDs for the specified file type")
            return SRObjectArray([])
        }
        
        // Retrieve all URLs for the app identified by a bundle id
        appURLs = bundleIds.compactMap { bundleId -> URL? in
            guard let retVal = LSCopyApplicationURLsForBundleIdentifier(bundleId as CFString, nil) else {
                return nil
            }
            return retVal.takeRetainedValue() as? URL
        }
    }
    
    return SRObjectArray(
        appURLs.compactMap { url -> NSObject? in
            guard !url.path.contains("/Applications/"),
                  let infoDict = Bundle(url: url)?.infoDictionary,
                  let name = (infoDict["CFBundleDisplayName"] ?? infoDict["CFBundleName"]) as? String,
                  let appId = infoDict["CFBundleIdentifier"] as? String
            else {
                return nil
            }
            
            let icon = NSWorkspace.shared.icon(forFile: url.path)
            
            return OpenWithApplication(
                name: SRString(name),
                id: SRString(appId),
                url: SRString(url.path),
                icon: SRData([UInt8](icon.png ?? Data()))
            )
        })
}

@_cdecl("open_file_path_with")
func openFilePathsWith(filePath: SRString, withUrl: SRString) {
    let config = NSWorkspace.OpenConfiguration()
    let at = URL(fileURLWithPath: withUrl.toString())
    
    // FIX-ME(HACK): The NULL split here is because I was not able to make this function accept a SRArray<SRString> argument.
    // So, considering these are file paths, and \0 is not a valid character for a file path,
    // I am using it as a delimitor to allow the rust side to pass in an array of files paths to this function
    let fileURLs = filePath.toString().split(separator: "\0").map {
        filePath in URL(fileURLWithPath: String(filePath))
    }
    
    NSWorkspace.shared.open(fileURLs, withApplicationAt: at, configuration: config)
}
