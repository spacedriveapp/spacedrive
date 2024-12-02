//
//  NativeFunctions.swift
//  Spacedrive
//
//  Created by Arnab Chakraborty on November 27, 2024.
//

import Foundation
import UIKit
import QuickLook

@objc(NativeFunctions)
class NativeFunctions: NSObject, QLPreviewControllerDataSource {
    private var fileURL: URL?

    @objc
    static func requiresMainQueueSetup() -> Bool {
        return true
    }

    private func getBookmarkStoragePath(for id: Int) -> URL {
        let documentsDirectory = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        return documentsDirectory.appendingPathComponent("\(id).sd_bookmark")
    }

    @objc
    func saveLocation(_ path: String,
                     locationId: NSNumber,
                     resolver resolve: @escaping RCTPromiseResolveBlock,
                     rejecter reject: @escaping RCTPromiseRejectBlock) {
        do {
            let url = URL(fileURLWithPath: path)
            guard url.startAccessingSecurityScopedResource() else {
                reject("ERROR", "Cannot access directory", nil)
                return
            }
            defer { url.stopAccessingSecurityScopedResource() }

            let bookmarkData = try url.bookmarkData(
                options: .minimalBookmark,
                includingResourceValuesForKeys: nil,
                relativeTo: nil
            )

            let bookmarkPath = getBookmarkStoragePath(for: locationId.intValue)
            try bookmarkData.write(to: bookmarkPath, options: .atomicWrite)

            resolve(["success": true])
        } catch {
            reject("ERROR", "Failed to create bookmark: \(error.localizedDescription)", nil)
        }
    }

    @objc
    func previewFile(_ path: String,
                     locationId: NSNumber,
                     resolver resolve: @escaping RCTPromiseResolveBlock,
                     rejecter reject: @escaping RCTPromiseRejectBlock) {
        #if DEBUG
        print("🔍 PreviewFile called with path: \(path), locationId: \(locationId)")
        #endif

        do {
            let bookmarkPath = getBookmarkStoragePath(for: locationId.intValue)
            #if DEBUG
            print("📁 Bookmark path: \(bookmarkPath)")
            #endif

            let fileURL = URL(fileURLWithPath: path)
            #if DEBUG
            print("📄 File URL: \(fileURL)")
            #endif

            if FileManager.default.fileExists(atPath: bookmarkPath.path) {
                #if DEBUG
                print("✅ Bookmark exists at path")
                #endif
                let bookmarkData = try Data(contentsOf: bookmarkPath)
                #if DEBUG
                print("📊 Bookmark data size: \(bookmarkData.count) bytes")
                #endif

                var isStale = false
                let directoryURL = try URL(
                    resolvingBookmarkData: bookmarkData,
                    options: [],
                    relativeTo: nil,
                    bookmarkDataIsStale: &isStale
                )
                #if DEBUG
                print("📂 Resolved directory URL: \(directoryURL)")
                print("🔄 Is bookmark stale? \(isStale)")
                #endif

                guard directoryURL.startAccessingSecurityScopedResource() else {
                    #if DEBUG
                    print("❌ Failed to access security-scoped resource for directory")
                    #endif
                    reject("ERROR", "Cannot access directory", nil)
                    return
                }
                defer {
                    directoryURL.stopAccessingSecurityScopedResource()
                    #if DEBUG
                    print("🔒 Stopped accessing security-scoped resource")
                    #endif
                }

                // Get the relative path from the base directory to the file
                let basePath = directoryURL.path
                let fullPath = fileURL.path

                #if DEBUG
                print("📍 Base path: \(basePath)")
                print("📍 Full path: \(fullPath)")
                #endif

                // Ensure the file path starts with the base path
                guard fullPath.hasPrefix(basePath) else {
                    #if DEBUG
                    print("❌ File is not within the bookmarked directory")
                    #endif
                    reject("ERROR", "File is not within the bookmarked directory", nil)
                    return
                }

                // Use the full file URL directly
                self.fileURL = fileURL
                #if DEBUG
                print("💾 Set fileURL for QuickLook: \(fileURL)")
                #endif

                // Verify file exists
                if FileManager.default.fileExists(atPath: fileURL.path) {
                    #if DEBUG
                    print("✅ File exists at path")
                    #endif
                } else {
                    #if DEBUG
                    print("⚠️ File does not exist at path")
                    #endif
                    reject("ERROR", "File not found at path", nil)
                    return
                }
            } else {
                #if DEBUG
                print("❌ Bookmark not found at path: \(bookmarkPath)")
                #endif
                reject("ERROR", "Bookmark not found for this location", nil)
                return
            }

            #if DEBUG
            print("🚀 Preparing to present QuickLook controller")
            #endif
            DispatchQueue.main.async {
                let previewController = QLPreviewController()
                previewController.dataSource = self

                guard let presentedVC = RCTPresentedViewController() else {
                    #if DEBUG
                    print("❌ Failed to get presented view controller")
                    #endif
                    reject("ERROR", "Cannot present preview", nil)
                    return
                }

                #if DEBUG
                print("📱 Presenting QuickLook controller")
                #endif
                presentedVC.present(previewController, animated: true) {
                    #if DEBUG
                    print("✨ QuickLook controller presented successfully")
                    #endif
                    resolve(["success": true])
                }
            }
        } catch {
            #if DEBUG
            print("💥 Error occurred: \(error.localizedDescription)")
            print("🔍 Detailed error: \(error)")
            #endif
            reject("ERROR", "Failed to preview file: \(error.localizedDescription)", nil)
        }
    }

    // MARK: - QLPreviewControllerDataSource
    func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
        #if DEBUG
        print("📊 numberOfPreviewItems called, returning: \(fileURL != nil ? 1 : 0)")
        #endif
        return fileURL != nil ? 1 : 0
    }

    func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
        #if DEBUG
        print("🎯 previewItemAt called for index: \(index)")
        print("📄 Returning fileURL: \(String(describing: fileURL))")
        #endif
        return fileURL! as QLPreviewItem
    }
}
