import AppKit
import SwiftRs

// Forward declaration of Rust callback
@_silgen_name("rust_drag_ended_callback")
func rust_drag_ended_callback(_ sessionId: UnsafePointer<CChar>, _ wasDropped: Bool)

private var activeDragSources: [String: NativeDragSource] = [:]
private var dragSourcesLock = NSLock()

@_cdecl("begin_native_drag")
public func beginNativeDrag(
    window: NSWindow,
    items: SRString,
    overlayWindow: NSWindow,
    sessionId: SRString
) -> Bool {
    guard let sourceView = window.contentView else {
        print("[DRAG] No content view found")
        return false
    }

    let sessionIdStr = sessionId.toString()
    let itemsJson = items.toString()

    print("[DRAG] Received items JSON: \(itemsJson)")

    guard let itemsData = itemsJson.data(using: .utf8) else {
        print("[DRAG] Failed to convert JSON string to data")
        return false
    }

    do {
        let itemArray = try JSONDecoder().decode([DragItemSpec].self, from: itemsData)
        print("[DRAG] Successfully decoded \(itemArray.count) items")
    } catch {
        print("[DRAG] Failed to decode items JSON: \(error)")
        return false
    }

    guard let itemArray = try? JSONDecoder().decode([DragItemSpec].self, from: itemsData) else {
        print("[DRAG] Failed to decode items JSON (second attempt)")
        return false
    }

    print("[DRAG] Starting drag with \(itemArray.count) items")

    let dragSource = NativeDragSource(
        sessionId: sessionIdStr,
        overlay: overlayWindow
    )

    dragSourcesLock.lock()
    activeDragSources[sessionIdStr] = dragSource
    dragSourcesLock.unlock()

    // Get the current mouse event, or create a synthetic one
    var mouseEvent = NSApp.currentEvent

    if mouseEvent == nil || mouseEvent!.type != .leftMouseDown {
        print("[DRAG] No valid mouse event, creating synthetic event")
        // Get current mouse location
        let mouseLocation = NSEvent.mouseLocation
        let windowLocation = window.convertPoint(fromScreen: mouseLocation)

        // Create a synthetic mouse down event
        mouseEvent = NSEvent.mouseEvent(
            with: .leftMouseDragged,
            location: windowLocation,
            modifierFlags: [],
            timestamp: ProcessInfo.processInfo.systemUptime,
            windowNumber: window.windowNumber,
            context: nil,
            eventNumber: 0,
            clickCount: 1,
            pressure: 1.0
        )
    }

    guard let event = mouseEvent else {
        print("[DRAG] Could not get or create mouse event")
        return false
    }

    let pasteboardItems = itemArray.compactMap { spec -> NSDraggingItem? in
        dragSource.createDraggingItem(for: spec)
    }

    guard !pasteboardItems.isEmpty else {
        print("[DRAG] No valid pasteboard items created")
        return false
    }

    print("[DRAG] Starting dragging session with \(pasteboardItems.count) items")

    let session = sourceView.beginDraggingSession(
        with: pasteboardItems,
        event: event,
        source: dragSource
    )

    session.draggingFormation = .none
    session.animatesToStartingPositionsOnCancelOrFail = true

    dragSource.session = session
    overlayWindow.orderFront(nil)
    overlayWindow.makeKeyAndOrderFront(nil)

    print("[DRAG] Drag session started successfully")

    return true
}

@_cdecl("end_native_drag")
public func endNativeDrag(sessionId: SRString) {
    let sessionIdStr = sessionId.toString()
    print("[DRAG] end_native_drag called for session: \(sessionIdStr)")

    dragSourcesLock.lock()
    if let source = activeDragSources.removeValue(forKey: sessionIdStr) {
        dragSourcesLock.unlock()
        print("[DRAG] Cleaning up drag source for session: \(sessionIdStr)")
        source.cleanup()
    } else {
        dragSourcesLock.unlock()
        print("[DRAG] No active drag source found for session: \(sessionIdStr)")
    }
}

@_cdecl("update_drag_overlay_position")
public func updateDragOverlayPosition(sessionId: SRString, x: Double, y: Double) {
    let sessionIdStr = sessionId.toString()

    dragSourcesLock.lock()
    guard let source = activeDragSources[sessionIdStr] else {
        dragSourcesLock.unlock()
        return
    }
    dragSourcesLock.unlock()

    let screenPoint = NSPoint(x: x, y: y)
    let frame = source.overlay.frame
    let centeredPoint = NSPoint(
        x: screenPoint.x - frame.width / 2,
        y: screenPoint.y - frame.height / 2
    )
    source.overlay.setFrameOrigin(centeredPoint)
}

struct DragItemSpec: Codable {
    let kind: DragItemKind
    let id: String
}

enum DragItemKind: Codable {
    case file(path: String)
    case filePromise(name: String, mimeType: String)
    case text(content: String)

    enum CodingKeys: String, CodingKey {
        case type
        case path
        case name
        case mimeType = "mime_type"
        case content
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)

        switch type {
        case "file":
            let path = try container.decode(String.self, forKey: .path)
            self = .file(path: path)
        case "filePromise":
            let name = try container.decode(String.self, forKey: .name)
            let mimeType = try container.decode(String.self, forKey: .mimeType)
            self = .filePromise(name: name, mimeType: mimeType)
        case "text":
            let content = try container.decode(String.self, forKey: .content)
            self = .text(content: content)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unknown drag item type: \(type)"
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)

        switch self {
        case .file(let path):
            try container.encode("file", forKey: .type)
            try container.encode(path, forKey: .path)
        case .filePromise(let name, let mimeType):
            try container.encode("filePromise", forKey: .type)
            try container.encode(name, forKey: .name)
            try container.encode(mimeType, forKey: .mimeType)
        case .text(let content):
            try container.encode("text", forKey: .type)
            try container.encode(content, forKey: .content)
        }
    }
}

class NativeDragSource: NSObject, NSDraggingSource, NSFilePromiseProviderDelegate {
    let sessionId: String
    let overlay: NSWindow
    weak var session: NSDraggingSession?

    private var filePromises: [String: DragItemSpec] = [:]

    init(
        sessionId: String,
        overlay: NSWindow
    ) {
        self.sessionId = sessionId
        self.overlay = overlay
        super.init()
    }

    func createDraggingItem(for spec: DragItemSpec) -> NSDraggingItem? {
        let writer: NSPasteboardWriting

        switch spec.kind {
        case .file(let path):
            let url = URL(fileURLWithPath: path)
            writer = url as NSPasteboardWriting

        case .filePromise(let name, let mimeType):
            let fileType: String
            if mimeType.hasPrefix("image/") {
                fileType = "public.image"
            } else if mimeType.hasPrefix("video/") {
                fileType = "public.movie"
            } else if mimeType.hasPrefix("audio/") {
                fileType = "public.audio"
            } else if mimeType == "application/pdf" {
                fileType = "com.adobe.pdf"
            } else {
                fileType = "public.data"
            }

            let provider = NSFilePromiseProvider(fileType: fileType, delegate: self)
            provider.userInfo = spec.id
            filePromises[spec.id] = spec
            writer = provider

        case .text(let content):
            let item = NSPasteboardItem()
            item.setString(content, forType: .string)
            writer = item
        }

        let draggingItem = NSDraggingItem(pasteboardWriter: writer)

        let transparentImage = NSImage(size: NSSize(width: 1, height: 1))
        draggingItem.setDraggingFrame(
            NSRect(origin: .zero, size: NSSize(width: 1, height: 1)),
            contents: transparentImage
        )

        return draggingItem
    }

    func draggingSession(
        _ session: NSDraggingSession,
        sourceOperationMaskFor context: NSDraggingContext
    ) -> NSDragOperation {
        return [.copy, .move]
    }

    func draggingSession(_ session: NSDraggingSession, movedTo screenPoint: NSPoint) {
        let frame = overlay.frame
        let centeredPoint = NSPoint(
            x: screenPoint.x - frame.width / 2,
            y: screenPoint.y - frame.height / 2
        )
        overlay.setFrameOrigin(centeredPoint)

        NotificationCenter.default.post(
            name: Notification.Name("spacedrive.drag.moved"),
            object: nil,
            userInfo: [
                "sessionId": sessionId,
                "x": screenPoint.x,
                "y": screenPoint.y
            ]
        )
    }

    func draggingSession(
        _ session: NSDraggingSession,
        endedAt screenPoint: NSPoint,
        operation: NSDragOperation
    ) {
        let eventType: String
        let wasDropped: Bool
        if operation.rawValue == 0 {
            eventType = "cancelled"
            wasDropped = false
        } else {
            eventType = "dropped"
            wasDropped = true
        }

        print("[DRAG] Drag session ended: session=\(sessionId), type=\(eventType), operation=\(operation.rawValue)")

        // Call Rust callback to notify that drag ended
        sessionId.withCString { cString in
            rust_drag_ended_callback(cString, wasDropped)
        }

        NotificationCenter.default.post(
            name: Notification.Name("spacedrive.drag.ended"),
            object: nil,
            userInfo: [
                "sessionId": sessionId,
                "type": eventType,
                "x": screenPoint.x,
                "y": screenPoint.y
            ]
        )

        dragSourcesLock.lock()
        let removed = activeDragSources.removeValue(forKey: sessionId)
        dragSourcesLock.unlock()

        if removed != nil {
            print("[DRAG] Removed drag source from active sources: \(sessionId)")
        } else {
            print("[DRAG] WARNING: Drag source not found in active sources: \(sessionId)")
        }

        cleanup()
        print("[DRAG] Cleanup completed for session: \(sessionId)")
    }

    func filePromiseProvider(
        _ filePromiseProvider: NSFilePromiseProvider,
        fileNameForType fileType: String
    ) -> String {
        guard let specId = filePromiseProvider.userInfo as? String,
              let spec = filePromises[specId],
              case .filePromise(let name, _) = spec.kind else {
            return "Untitled"
        }
        return name
    }

    func filePromiseProvider(
        _ filePromiseProvider: NSFilePromiseProvider,
        writePromiseTo url: URL,
        completionHandler: @escaping (Error?) -> Void
    ) {
        guard let specId = filePromiseProvider.userInfo as? String,
              let spec = filePromises[specId] else {
            completionHandler(NSError(
                domain: "com.spacedrive.drag",
                code: -1,
                userInfo: [NSLocalizedDescriptionKey: "File promise not found"]
            ))
            return
        }

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let notificationName = Notification.Name("spacedrive.drag.filePromiseRequested")
                let userInfo = [
                    "sessionId": self.sessionId,
                    "itemId": spec.id,
                    "destinationUrl": url.path
                ]

                NotificationCenter.default.post(
                    name: notificationName,
                    object: nil,
                    userInfo: userInfo
                )

                let semaphore = DispatchSemaphore(value: 0)
                var writeError: Error?

                let observer = NotificationCenter.default.addObserver(
                    forName: Notification.Name("spacedrive.drag.filePromiseWritten"),
                    object: nil,
                    queue: nil
                ) { notification in
                    if let info = notification.userInfo,
                       let itemId = info["itemId"] as? String,
                       itemId == spec.id {
                        if let error = info["error"] as? String {
                            writeError = NSError(
                                domain: "com.spacedrive.drag",
                                code: -1,
                                userInfo: [NSLocalizedDescriptionKey: error]
                            )
                        }
                        semaphore.signal()
                    }
                }

                let timeout = semaphore.wait(timeout: .now() + 30.0)
                NotificationCenter.default.removeObserver(observer)

                if timeout == .timedOut {
                    throw NSError(
                        domain: "com.spacedrive.drag",
                        code: -1,
                        userInfo: [NSLocalizedDescriptionKey: "File promise write timeout"]
                    )
                }

                completionHandler(writeError)
            } catch {
                completionHandler(error)
            }
        }
    }

    func cleanup() {
        overlay.orderOut(nil)
        filePromises.removeAll()
    }
}
