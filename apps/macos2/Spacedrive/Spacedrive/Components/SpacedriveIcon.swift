import SwiftUI

/// Enum representing all available Spacedrive icons
/// This enum provides type-safe access to all icons from the Spacedrive v1 assets
enum SpacedriveIcon: String, CaseIterable {
    // File Type Icons
    case album = "Album"
    case albumLight = "Album_Light"
    case album20 = "Album-20"
    case alias = "Alias"
    case aliasLight = "Alias_Light"
    case alias20 = "Alias-20"
    case application = "Application"
    case applicationLight = "Application_Light"
    case archive = "Archive"
    case archiveLight = "Archive_Light"
    case archive20 = "Archive-20"
    case audio = "Audio"
    case audioLight = "Audio_Light"
    case audio20 = "Audio-20"
    case book = "Book"
    case bookLight = "Book_Light"
    case book20 = "Book-20"
    case bookBlue = "BookBlue"
    case code20 = "Code-20"
    case collection = "Collection"
    case collectionLight = "Collection_Light"
    case collection20 = "Collection-20"
    case collectionSparkle = "CollectionSparkle"
    case collectionSparkleLight = "CollectionSparkle_Light"
    case config20 = "Config-20"
    case database = "Database"
    case databaseLight = "Database_Light"
    case database20 = "Database-20"
    case document = "Document"
    case documentLight = "Document_Light"
    case document20 = "Document-20"
    case documentDoc = "Document_doc"
    case documentDocLight = "Document_doc_Light"
    case documentPdf = "Document_pdf"
    case documentPdfLight = "Document_pdf_Light"
    case documentSrt = "Document_srt"
    case documentXls = "Document_xls"
    case documentXlsLight = "Document_xls_Light"
    case documentXmp = "Document_xmp"
    case dotfile20 = "Dotfile-20"
    case encrypted = "Encrypted"
    case encryptedLight = "Encrypted_Light"
    case encrypted20 = "Encrypted-20"
    case entity = "Entity"
    case entityLight = "Entity_Light"
    case executable = "Executable"
    case executableLight = "Executable_Light"
    case executableLightOld = "Executable_Light_old"
    case executableOld = "Executable_old"
    case executable20 = "Executable-20"
    case faceLight = "Face_Light"
    case folder = "Folder"
    case folderLight = "Folder_Light"
    case folder20 = "Folder-20"
    case folderTagXmp = "Folder-tag-xmp"
    case folderGrey = "FolderGrey"
    case folderGreyLight = "FolderGrey_Light"
    case folderNoSpace = "FolderNoSpace"
    case folderNoSpaceLight = "FolderNoSpace_Light"
    case font20 = "Font-20"
    case game = "Game"
    case gameLight = "Game_Light"
    case image = "Image"
    case imageLight = "Image_Light"
    case image20 = "Image-20"
    case key = "Key"
    case keyLight = "Key_Light"
    case key20 = "Key-20"
    case keys = "Keys"
    case keysLight = "Keys_Light"
    case link = "Link"
    case linkLight = "Link_Light"
    case link20 = "Link-20"
    case lock = "Lock"
    case lockLight = "Lock_Light"
    case mesh = "Mesh"
    case meshLight = "Mesh_Light"
    case mesh20 = "Mesh-20"
    case movie = "Movie"
    case movieLight = "Movie_Light"
    case package = "Package"
    case packageLight = "Package_Light"
    case package20 = "Package-20"
    case screenshot = "Screenshot"
    case screenshotLight = "Screenshot_Light"
    case screenshot20 = "Screenshot-20"
    case screenshotAlt = "ScreenshotAlt"
    case text = "Text"
    case textLight = "Text_Light"
    case text20 = "Text-20"
    case textAlt = "TextAlt"
    case textAltLight = "TextAlt_Light"
    case textTxt = "Text_txt"
    case texturedMesh = "TexturedMesh"
    case texturedMeshLight = "TexturedMesh_Light"
    case trash = "Trash"
    case trashLight = "Trash_Light"
    case undefined = "Undefined"
    case undefinedLight = "Undefined_Light"
    case unknown20 = "Unknown-20"
    case video = "Video"
    case videoLight = "Video_Light"
    case video20 = "Video-20"
    case webPageArchive20 = "WebPageArchive-20"
    case widget = "Widget"
    case widgetLight = "Widget_Light"
    case widget20 = "Widget-20"

    // Drive/Cloud Service Icons
    case amazonS3 = "AmazonS3"
    case androidPhotos = "AndroidPhotos"
    case appleFiles = "AppleFiles"
    case applePhotos = "ApplePhotos"
    case backBlaze = "BackBlaze"
    case box = "Box"
    case cloudSync = "CloudSync"
    case cloudSyncLight = "CloudSync_Light"
    case dav = "DAV"
    case deleteLocation = "DeleteLocation"
    case drive = "Drive"
    case driveLight = "Drive_Light"
    case driveAmazonS3 = "Drive-AmazonS3"
    case driveAmazonS3Light = "Drive-AmazonS3_Light"
    case driveBackBlaze = "Drive-BackBlaze"
    case driveBackBlazeLight = "Drive-BackBlaze_Light"
    case driveBox = "Drive-Box"
    case driveBoxLight = "Drive-box_Light"
    case driveDarker = "Drive-Darker"
    case driveDav = "Drive-DAV"
    case driveDavLight = "Drive-DAV_Light"
    case driveDropbox = "Drive-Dropbox"
    case driveDropboxLight = "Drive-Dropbox_Light"
    case driveGoogleDrive = "Drive-GoogleDrive"
    case driveGoogleDriveLight = "Drive-GoogleDrive_Light"
    case driveMega = "Drive-Mega"
    case driveMegaLight = "Drive-Mega_Light"
    case driveOneDrive = "Drive-OneDrive"
    case driveOneDriveLight = "Drive-OneDrive_Light"
    case driveOpenStack = "Drive-OpenStack"
    case driveOpenStackLight = "Drive-OpenStack_Light"
    case drivePCloud = "Drive-PCloud"
    case drivePCloudLight = "Drive-PCloud_Light"
    case dropbox = "Dropbox"
    case googleDrive = "GoogleDrive"
    case location = "Location"
    case locationManaged = "LocationManaged"
    case locationReplica = "LocationReplica"
    case mega = "Mega"
    case moveLocation = "MoveLocation"
    case moveLocationLight = "MoveLocation_Light"
    case newLocation = "NewLocation"
    case node = "Node"
    case nodeLight = "Node_Light"
    case oneDrive = "OneDrive"
    case openStack = "OpenStack"
    case pCloud = "PCloud"
    case spacedrop = "Spacedrop"
    case spacedropLight = "Spacedrop_Light"
    case spacedrop1 = "Spacedrop-1"
    case sync = "Sync"
    case syncLight = "Sync_Light"

    // Device Icons
    case ball = "Ball"
    case globe = "Globe"
    case globeLight = "Globe_Light"
    case globeAlt = "GlobeAlt"
    case hdd = "HDD"
    case hddLight = "HDD_Light"
    case heart = "Heart"
    case heartLight = "Heart_Light"
    case home = "Home"
    case homeLight = "Home_Light"
    case laptop = "Laptop"
    case laptopLight = "Laptop_Light"
    case mobile = "Mobile"
    case mobileLight = "Mobile_Light"
    case mobileAndroid = "Mobile-Android"
    case miniSilverBox = "MiniSilverBox"
    case pc = "PC"
    case scrapbook = "Scrapbook"
    case scrapbookLight = "Scrapbook_Light"
    case sd = "SD"
    case sdLight = "SD_Light"
    case search = "Search"
    case searchLight = "Search_Light"
    case searchAlt = "SearchAlt"
    case server = "Server"
    case serverLight = "Server_Light"
    case silverBox = "SilverBox"
    case tablet = "Tablet"
    case tabletLight = "Tablet_Light"
    case tags = "Tags"
    case tagsLight = "Tags_Light"
    case terminal = "Terminal"
    case terminalLight = "Terminal_Light"

    /// Returns the filename for the icon (without extension)
    var filename: String {
        return rawValue
    }

    /// Returns the full filename with .png extension
    var fullFilename: String {
        return "\(rawValue).png"
    }

    /// Returns a user-friendly display name for the icon
    var displayName: String {
        return rawValue.replacingOccurrences(of: "_", with: " ")
            .replacingOccurrences(of: "-", with: " ")
            .capitalized
    }

    /// Returns true if this is a light theme variant
    var isLightVariant: Bool {
        return rawValue.hasSuffix("_Light")
    }

    /// Returns true if this is a 20px variant
    var is20pxVariant: Bool {
        return rawValue.hasSuffix("-20")
    }

    /// Returns the base name without variant suffixes
    var baseName: String {
        var name = rawValue
        if name.hasSuffix("_Light") {
            name = String(name.dropLast(6)) // Remove "_Light"
        }
        if name.hasSuffix("-20") {
            name = String(name.dropLast(3)) // Remove "-20"
        }
        return name
    }
}

/// SwiftUI view component for displaying Spacedrive icons
struct SpacedriveIconView: View {
    let icon: SpacedriveIcon
    let size: CGFloat
    let color: Color?

    init(_ icon: SpacedriveIcon, size: CGFloat = 16, color: Color? = nil) {
        self.icon = icon
        self.size = size
        self.color = color
    }

    var body: some View {
        if let bundlePath = Bundle.main.path(forResource: "Spacedrive_Spacedrive", ofType: "bundle"),
           let bundle = Bundle(path: bundlePath),
           let imagePath = bundle.path(forResource: icon.filename, ofType: "png"),
           let image = NSImage(contentsOfFile: imagePath)
        {
            Image(nsImage: image)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(width: size, height: size)
                .foregroundColor(color)
        } else {
            // Fallback for missing images
            Rectangle()
                .fill(Color.gray.opacity(0.3))
                .frame(width: size, height: size)
                .overlay(
                    Text("?")
                        .font(.caption)
                        .foregroundColor(SpacedriveColors.Text.secondary)
                )
        }
    }
}

/// Convenience extensions for common icon usage patterns
extension SpacedriveIcon {
    /// Get the appropriate icon variant based on theme and size preferences
    static func preferredVariant(
        baseName: String,
        isLightTheme: Bool = false,
        prefer20px: Bool = false
    ) -> SpacedriveIcon? {
        // Try to find the preferred variant
        let variants = [
            // 20px variants first if preferred
            prefer20px ? "\(baseName)-20" : nil,
            // Light variants if light theme
            isLightTheme ? "\(baseName)_Light" : nil,
            // Base variant
            baseName,
        ].compactMap { $0 }

        for variant in variants {
            if let icon = SpacedriveIcon(rawValue: variant) {
                return icon
            }
        }

        return nil
    }

    /// Get all variants of a base icon name
    static func allVariants(for baseName: String) -> [SpacedriveIcon] {
        return SpacedriveIcon.allCases.filter { $0.baseName == baseName }
    }
}

/// Preview for testing the icon component
struct SpacedriveIconView_Previews: PreviewProvider {
    static var previews: some View {
        VStack(spacing: 16) {
            Text("Spacedrive Icons")
                .font(.title)
                .padding()

            LazyVGrid(columns: Array(repeating: GridItem(.flexible()), count: 6), spacing: 12) {
                ForEach(Array(SpacedriveIcon.allCases.prefix(24)), id: \.self) { icon in
                    VStack(spacing: 4) {
                        SpacedriveIconView(icon, size: 24)
                        Text(icon.displayName)
                            .font(.caption2)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .padding(8)
                    .background(Color.gray.opacity(0.1))
                    .cornerRadius(8)
                }
            }
            .padding()
        }
    }
}
