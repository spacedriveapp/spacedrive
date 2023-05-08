import AppKit
import SwiftRs

class OpenWithApplication: NSObject {
    var name: SRString;
    var id: SRString;
    var url: SRString;
    
    init(name: SRString, id: SRString, url: SRString) {
        self.name = name
        self.id = id
        self.url = url
    }
}

@_cdecl("get_open_with_applications")
func getOpenWithApplications(urlString: SRString) -> SRObjectArray {
    let url: URL;
    
    if #available(macOS 13.0, *) {
        url = URL(filePath: urlString.toString())
    } else {
        // Fallback on earlier versions
        url = URL(fileURLWithPath: urlString.toString())
    }
    
                                       
    if #available(macOS 12.0, *) {
        return SRObjectArray(NSWorkspace.shared.urlsForApplications(toOpen: url)
            .compactMap { url in
                Bundle(url: url)?.infoDictionary.map { ($0, url) }
            }
            .compactMap { (dict, url) in
                let name = SRString((dict["CFBundleDisplayName"] ?? dict["CFBundleName"]) as! String);
                
                if !url.path.contains("/Applications/") {
                    return nil
                }
                
                return OpenWithApplication(
                    name: name,
                    id: SRString(dict["CFBundleIdentifier"] as! String),
                    url: SRString(url.path)
                )
            })
    } else {
        // Fallback on earlier versions
        return SRObjectArray([])
    }
}

@_cdecl("open_file_path_with")
func openFilePathWith(fileUrl: SRString, withUrl: SRString) {
    let config = NSWorkspace.OpenConfiguration();

    let at = URL(fileURLWithPath: withUrl.toString());
    print(at);
    
    NSWorkspace.shared.open(
        [URL(fileURLWithPath: fileUrl.toString())],
        withApplicationAt: at,
        configuration: config
    )
    
//    NSWorkspace.shared.openApplication(at: at, configuration: config) { (app, err) in
//        print(app)
//        print(err)
//    }
}
