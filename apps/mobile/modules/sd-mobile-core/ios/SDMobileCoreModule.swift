import ExpoModulesCore

// C function declarations from Rust
@_silgen_name("initialize_core")
func initialize_core(data_dir: UnsafePointer<CChar>, device_name: UnsafePointer<CChar>?) -> Int32

@_silgen_name("handle_core_msg")
func handle_core_msg(
    query: UnsafePointer<CChar>,
    callback: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void,
    callback_data: UnsafeMutableRawPointer?
)

@_silgen_name("spawn_core_event_listener")
func spawn_core_event_listener(
    callback: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void,
    callback_data: UnsafeMutableRawPointer?
)

@_silgen_name("spawn_core_log_listener")
func spawn_core_log_listener(
    callback: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void,
    callback_data: UnsafeMutableRawPointer?
)

@_silgen_name("shutdown_core")
func shutdown_core()

// Promise wrapper for async callbacks
private class SwiftPromise {
    var promise: Promise
    init(promise: Promise) {
        self.promise = promise
    }
}

// Callback for message responses
private func messageCallback(data: UnsafeMutableRawPointer?, result: UnsafePointer<CChar>) {
    guard let data = data else { return }
    let promise = Unmanaged<SwiftPromise>.fromOpaque(data).takeRetainedValue()
    let resultStr = String(cString: result)
    promise.promise.resolve(resultStr)
}

// Callback for events
private func eventCallback(data: UnsafeMutableRawPointer?, event: UnsafePointer<CChar>) {
    guard let data = data else { return }
    let module = Unmanaged<SDMobileCoreModule>.fromOpaque(data).takeUnretainedValue()
    let eventStr = String(cString: event)
    if module.listeners > 0 {
        module.sendEvent("SDCoreEvent", ["body": eventStr])
    }
}

// Callback for logs
private func logCallback(data: UnsafeMutableRawPointer?, log: UnsafePointer<CChar>) {
    guard let data = data else { return }
    let module = Unmanaged<SDMobileCoreModule>.fromOpaque(data).takeUnretainedValue()
    let logStr = String(cString: log)
    if module.logListeners > 0 {
        module.sendEvent("SDCoreLog", ["body": logStr])
    }
}

// Expo Module
public class SDMobileCoreModule: Module {
    var listeners = 0
    var logListeners = 0
    private var registeredWithRust = false
    private var logRegisteredWithRust = false

    public func definition() -> ModuleDefinition {
        Name("SDMobileCore")

        Events("SDCoreEvent", "SDCoreLog")

        OnStartObserving("SDCoreEvent") {
            NSLog("[SDMobileCore] 📡 OnStartObserving SDCoreEvent triggered")

            // Register event listener if not already done
            if !self.registeredWithRust {
                NSLog("[SDMobileCore] 🚀 Registering event listener...")
                spawn_core_event_listener(
                    callback: eventCallback,
                    callback_data: Unmanaged.passUnretained(self).toOpaque()
                )
                self.registeredWithRust = true
                NSLog("[SDMobileCore] ✅ Event listener registered with Rust")
            }

            self.listeners += 1
            NSLog("[SDMobileCore] 📊 SDCoreEvent listeners: \(self.listeners)")
        }

        OnStopObserving("SDCoreEvent") {
            self.listeners -= 1
            NSLog("[SDMobileCore] 📉 SDCoreEvent listeners: \(self.listeners)")
        }

        OnStartObserving("SDCoreLog") {
            NSLog("[SDMobileCore] 📡 OnStartObserving SDCoreLog triggered")

            // Register log listener if not already done
            if !self.logRegisteredWithRust {
                NSLog("[SDMobileCore] 🚀 Registering log listener...")
                spawn_core_log_listener(
                    callback: logCallback,
                    callback_data: Unmanaged.passUnretained(self).toOpaque()
                )
                self.logRegisteredWithRust = true
                NSLog("[SDMobileCore] ✅ Log listener registered with Rust")
            }

            self.logListeners += 1
            NSLog("[SDMobileCore] 📊 SDCoreLog listeners: \(self.logListeners)")
        }

        OnStopObserving("SDCoreLog") {
            self.logListeners -= 1
            NSLog("[SDMobileCore] 📉 SDCoreLog listeners: \(self.logListeners)")
        }

        Function("initialize") { (dataDir: String?, deviceName: String?) throws -> Int in
            // Use app support directory if no data dir provided
            let dir: String
            if let dataDir = dataDir {
                dir = dataDir
            } else {
                let paths = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
                dir = paths[0].appendingPathComponent("SpacedriveData").path
            }

            print("[SDMobileCore] Using data directory: \(dir)")

            // Ensure directory exists
            do {
                try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true, attributes: nil)
                NSLog("[SDMobileCore] ✅ Data directory created/verified: %@", dir)
            } catch {
                NSLog("[SDMobileCore] ❌ FAILED to create directory: %@", error.localizedDescription)
                throw NSError(domain: "SDMobileCore", code: -2, userInfo: [
                    NSLocalizedDescriptionKey: "Failed to create data directory: \(error.localizedDescription)"
                ])
            }

            // Verify directory is writable
            let testFile = (dir as NSString).appendingPathComponent("test.tmp")
            do {
                try "test".write(toFile: testFile, atomically: true, encoding: .utf8)
                try FileManager.default.removeItem(atPath: testFile)
                NSLog("[SDMobileCore] ✅ Directory is writable")
            } catch {
                NSLog("[SDMobileCore] ❌ Directory is NOT writable: %@", error.localizedDescription)
                throw NSError(domain: "SDMobileCore", code: -3, userInfo: [
                    NSLocalizedDescriptionKey: "Data directory is not writable: \(error.localizedDescription)"
                ])
            }

            NSLog("[SDMobileCore] 🚀 Calling Rust initialize_core...")
            let result = dir.withCString { dirPtr in
                if let deviceName = deviceName {
                    return Int(deviceName.withCString { namePtr in
                        initialize_core(data_dir: dirPtr, device_name: namePtr)
                    })
                } else {
                    return Int(initialize_core(data_dir: dirPtr, device_name: nil))
                }
            }
            NSLog("[SDMobileCore] 📊 Rust initialize_core returned: %d", result)

            if result != 0 {
                throw NSError(domain: "SDMobileCore", code: result, userInfo: [
                    NSLocalizedDescriptionKey: "Rust core initialization failed with code \(result). Check console logs for details from Core::new_with_config()"
                ])
            }

            return result
        }

        AsyncFunction("sendMessage") { (query: String, promise: Promise) in
            let promiseWrapper = SwiftPromise(promise: promise)
            let promisePtr = Unmanaged.passRetained(promiseWrapper).toOpaque()

            query.withCString { queryPtr in
                handle_core_msg(
                    query: queryPtr,
                    callback: messageCallback,
                    callback_data: promisePtr
                )
            }
        }

        Function("shutdown") {
            shutdown_core()
        }
    }
}
