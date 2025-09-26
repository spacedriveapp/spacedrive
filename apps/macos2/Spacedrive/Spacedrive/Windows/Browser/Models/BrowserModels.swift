import Foundation
import SwiftUI
import Combine

/// Represents a location/folder in the browser
struct BrowserLocation: Identifiable, Hashable {
    let id = UUID()
    let name: String
    let path: String
    let iconName: String
    let isDirectory: Bool

    init(name: String, path: String, iconName: String, isDirectory: Bool = true) {
        self.name = name
        self.path = path
        self.iconName = iconName
        self.isDirectory = isDirectory
    }

    // Computed property for SwiftUI compatibility
    var icon: String { iconName }
}

/// Represents a file or folder item in the browser
struct BrowserItem: Identifiable, Hashable {
    let id = UUID()
    let name: String
    let path: String
    let icon: String
    let type: String
    let size: String
    let modifiedDate: String
    let isDirectory: Bool
}

/// Browser state management
@MainActor
class BrowserState: ObservableObject {
    @Published var selectedLocation: BrowserLocation?
    @Published var currentPath: String = "/"
    @Published var locations: [BrowserLocation] = []
    @Published var sidebarCollapsed: Bool = false
    @Published var hasNewNotifications: Bool = false

    // Image preview state
    @Published var previewImagePath: String?
    @Published var isShowingImagePreview: Bool = false

    // Computed properties for the new UI
    @Published var currentItems: [BrowserItem] = []
    @Published var recentItems: [BrowserItem] = []

    init() {
        setupDefaultLocations()
        setupSampleData()
    }

    private func setupDefaultLocations() {
        locations = [
            BrowserLocation(name: "Home", path: "/Users/jamespine", iconName: "house.fill"),
            BrowserLocation(name: "Desktop", path: "/Users/jamespine/Desktop", iconName: "desktopcomputer"),
            BrowserLocation(name: "Documents", path: "/Users/jamespine/Documents", iconName: "doc.fill"),
            BrowserLocation(name: "Downloads", path: "/Users/jamespine/Downloads", iconName: "arrow.down.circle.fill"),
            BrowserLocation(name: "Pictures", path: "/Users/jamespine/Pictures", iconName: "photo.fill"),
            BrowserLocation(name: "Music", path: "/Users/jamespine/Music", iconName: "music.note"),
            BrowserLocation(name: "Movies", path: "/Users/jamespine/Movies", iconName: "video.fill"),
            BrowserLocation(name: "Applications", path: "/Applications", iconName: "app.fill"),
            BrowserLocation(name: "Library", path: "/Users/jamespine/Library", iconName: "folder.fill"),
            BrowserLocation(name: "System", path: "/System", iconName: "gear.circle.fill"),
            BrowserLocation(name: "Volumes", path: "/Volumes", iconName: "externaldrive.fill")
        ]

        selectedLocation = locations.first
    }

    private func setupSampleData() {
        // Sample current items
        currentItems = [
            BrowserItem(name: "Project Files", path: "/Users/jamespine/Desktop/Project Files", icon: "folder", type: "Folder", size: "2.3 GB", modifiedDate: "Today", isDirectory: true),
            BrowserItem(name: "Screenshot.png", path: "/Users/jamespine/Desktop/Screenshot.png", icon: "photo", type: "PNG Image", size: "1.2 MB", modifiedDate: "Yesterday", isDirectory: false),
            BrowserItem(name: "Document.pdf", path: "/Users/jamespine/Desktop/Document.pdf", icon: "doc.text", type: "PDF Document", size: "856 KB", modifiedDate: "2 days ago", isDirectory: false),
            BrowserItem(name: "Video.mp4", path: "/Users/jamespine/Desktop/Video.mp4", icon: "video", type: "MP4 Video", size: "45.2 MB", modifiedDate: "3 days ago", isDirectory: false),
            BrowserItem(name: "Archive.zip", path: "/Users/jamespine/Desktop/Archive.zip", icon: "archivebox", type: "ZIP Archive", size: "12.8 MB", modifiedDate: "1 week ago", isDirectory: false),
            BrowserItem(name: "Code Project", path: "/Users/jamespine/Desktop/Code Project", icon: "folder.fill", type: "Folder", size: "156 MB", modifiedDate: "1 week ago", isDirectory: true)
        ]

        // Sample recent items
        recentItems = [
            BrowserItem(name: "Recent Document.docx", path: "/Users/jamespine/Documents/Recent Document.docx", icon: "doc.text.fill", type: "Word Document", size: "234 KB", modifiedDate: "1 hour ago", isDirectory: false),
            BrowserItem(name: "Presentation.key", path: "/Users/jamespine/Documents/Presentation.key", icon: "presentation", type: "Keynote Presentation", size: "8.7 MB", modifiedDate: "3 hours ago", isDirectory: false),
            BrowserItem(name: "Spreadsheet.xlsx", path: "/Users/jamespine/Documents/Spreadsheet.xlsx", icon: "tablecells", type: "Excel Spreadsheet", size: "1.1 MB", modifiedDate: "5 hours ago", isDirectory: false)
        ]

        // Simulate notifications
        Task {
            try? await Task.sleep(nanoseconds: 2_000_000_000) // 2 seconds
            hasNewNotifications = true
        }
    }

    func selectLocation(_ location: BrowserLocation) {
        selectedLocation = location
        currentPath = location.path
        // In a real app, this would load the actual files from the location
        loadItemsForLocation(location)
    }

    private func loadItemsForLocation(_ location: BrowserLocation) {
        // Simulate loading different items for different locations
        switch location.name {
        case "Pictures":
            currentItems = [
                BrowserItem(name: "Vacation Photos", path: "\(location.path)/Vacation Photos", icon: "photo.on.rectangle", type: "Folder", size: "2.1 GB", modifiedDate: "Today", isDirectory: true),
                BrowserItem(name: "Screenshot 2024.png", path: "\(location.path)/Screenshot 2024.png", icon: "photo", type: "PNG Image", size: "2.3 MB", modifiedDate: "Yesterday", isDirectory: false),
                BrowserItem(name: "Family Portrait.jpg", path: "\(location.path)/Family Portrait.jpg", icon: "photo", type: "JPEG Image", size: "4.7 MB", modifiedDate: "2 days ago", isDirectory: false)
            ]
        case "Documents":
            currentItems = [
                BrowserItem(name: "Work Projects", path: "\(location.path)/Work Projects", icon: "folder", type: "Folder", size: "1.8 GB", modifiedDate: "Today", isDirectory: true),
                BrowserItem(name: "Meeting Notes.txt", path: "\(location.path)/Meeting Notes.txt", icon: "doc.text", type: "Text Document", size: "12 KB", modifiedDate: "Yesterday", isDirectory: false),
                BrowserItem(name: "Budget.xlsx", path: "\(location.path)/Budget.xlsx", icon: "tablecells", type: "Excel Spreadsheet", size: "456 KB", modifiedDate: "3 days ago", isDirectory: false)
            ]
        default:
            // Use default sample data
            break
        }
    }

    func toggleSidebar() {
        sidebarCollapsed.toggle()
    }

    func showImagePreview(path: String) {
        previewImagePath = path
        isShowingImagePreview = true
    }

    func hideImagePreview() {
        previewImagePath = nil
        isShowingImagePreview = false
    }
}

