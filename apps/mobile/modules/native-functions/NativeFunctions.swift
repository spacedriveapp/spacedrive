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
        print("🔍 PreviewFile called with path: \(path), locationId: \(locationId)")
        
        do {
            let bookmarkPath = getBookmarkStoragePath(for: locationId.intValue)
            print("📁 Bookmark path: \(bookmarkPath)")
            
            let fileURL = URL(fileURLWithPath: path)
            print("📄 File URL: \(fileURL)")
            
            if FileManager.default.fileExists(atPath: bookmarkPath.path) {
                print("✅ Bookmark exists at path")
                let bookmarkData = try Data(contentsOf: bookmarkPath)
                print("📊 Bookmark data size: \(bookmarkData.count) bytes")
                
                var isStale = false
                let directoryURL = try URL(
                    resolvingBookmarkData: bookmarkData,
                    options: [],
                    relativeTo: nil,
                    bookmarkDataIsStale: &isStale
                )
                print("📂 Resolved directory URL: \(directoryURL)")
                print("🔄 Is bookmark stale? \(isStale)")
                
                guard directoryURL.startAccessingSecurityScopedResource() else {
                    print("❌ Failed to access security-scoped resource for directory")
                    reject("ERROR", "Cannot access directory", nil)
                    return
                }
                defer {
                    directoryURL.stopAccessingSecurityScopedResource()
                    print("🔒 Stopped accessing security-scoped resource")
                }
                
                let fileName = fileURL.lastPathComponent
                print("📝 File name: \(fileName)")
                
                let resolvedFileURL = directoryURL.appendingPathComponent(fileName)
                print("🎯 Resolved file URL: \(resolvedFileURL)")
                
                // Check if file exists at resolved path
                if FileManager.default.fileExists(atPath: resolvedFileURL.path) {
                    print("✅ File exists at resolved path")
                } else {
                    print("⚠️ File does not exist at resolved path")
                }
                
                self.fileURL = resolvedFileURL
                print("💾 Set fileURL for QuickLook: \(resolvedFileURL)")
            } else {
                print("❌ Bookmark not found at path: \(bookmarkPath)")
                reject("ERROR", "Bookmark not found for this location", nil)
                return
            }
            
            print("🚀 Preparing to present QuickLook controller")
            DispatchQueue.main.async {
                let previewController = QLPreviewController()
                previewController.dataSource = self
                
                guard let presentedVC = RCTPresentedViewController() else {
                    print("❌ Failed to get presented view controller")
                    reject("ERROR", "Cannot present preview", nil)
                    return
                }
                
                print("📱 Presenting QuickLook controller")
                presentedVC.present(previewController, animated: true) {
                    print("✨ QuickLook controller presented successfully")
                    resolve(["success": true])
                }
            }
        } catch {
            print("💥 Error occurred: \(error.localizedDescription)")
            print("🔍 Detailed error: \(error)")
            reject("ERROR", "Failed to preview file: \(error.localizedDescription)", nil)
        }
    }

    
    // MARK: - QLPreviewControllerDataSource
    func numberOfPreviewItems(in controller: QLPreviewController) -> Int {
        print("📊 numberOfPreviewItems called, returning: \(fileURL != nil ? 1 : 0)")
        return fileURL != nil ? 1 : 0
    }

    func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
        print("🎯 previewItemAt called for index: \(index)")
        print("📄 Returning fileURL: \(String(describing: fileURL))")
        return fileURL! as QLPreviewItem
    }
}
