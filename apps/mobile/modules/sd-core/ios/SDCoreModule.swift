import ExpoModulesCore

// A class wrapper around the `Promise` value type.
private class SwiftPromise {
    var p: Promise;

    init(promise: Promise) {
        self.p = promise;
    }
}

// Used by Rust to indicate an error started while setting up the core.
// This will be called within the call to `sd_init_core`.
@_cdecl("SDCoreModule_setCoreStartupError")
func sd_set_core_startup_error(sd_core_module: UnsafeRawPointer, len: UnsafePointer<CUnsignedInt>, buf: UnsafeMutableRawPointer) {
    // SAFTEY: We take as unretained because we only want to use this reference not consume it.
    // SAFTEY: Rust is still holding the reference so retaining could result in it being deallocated by Swift while Rust is still holding it.
    let sd_core_module = Unmanaged<SDCoreModule>.fromOpaque(sd_core_module).takeUnretainedValue();
        
    let buf = UnsafeRawBufferPointer(start: buf, count: Int(len.pointee));
    sd_core_module.coreStartupError = String(bytes: buf, encoding: String.Encoding.utf8);
    print("Spacedrive core startup error: \(sd_core_module.coreStartupError as Optional)");
}

// Used by Rust to emit an "sdCoreEvent" to the frontend.
@_cdecl("SDCoreModule_sdEmitEvent")
func sd_emit_event(this: UnsafeMutableRawPointer, len: UnsafePointer<CInt>, buf: UnsafeMutableRawPointer) {
//    // The error string was too long. This is an edge case and exists to avoid unwinding over FFI-boundary.
//    if buf == nil {
//        return;
//    }
    
    // SAFTEY: We take as unretained because we only want to use this reference not consume it.
    // SAFTEY: Rust is still holding the reference so retaining could result in it being deallocated by Swift while Rust is still holding it.
    let sd_core_module = Unmanaged<SDCoreModule>.fromOpaque(this).takeUnretainedValue();
    
    let buf = UnsafeRawBufferPointer(start: buf, count: Int(len.pointee));
    sd_core_module.sendEvent("sdCoreEvent", [
        "data": String(bytes: buf, encoding: String.Encoding.utf8)
    ])
}


// Used by Rust to response to an `sd_core_msg` call.
// This exists because it is an asynchronous function and over the FFI-boundary that is safest as a callback.
//
// SAFTEY: Rust should assume the `promise` pointer is deallocated after this function so it must not continue to hold it!
@_cdecl("SDCoreModule_sdCoreMsgResult")
func sd_core_msg_result(sd_core_module: UnsafeRawPointer, promise: UnsafeRawPointer, status: CInt, len: UnsafePointer<CInt>, buf: UnsafeMutableRawPointer?) {
    // SAFTEY: We take as unretained because we only want to use this reference not consume it.
    // SAFTEY: Rust is still holding the reference so retaining could result in it being deallocated by Swift while Rust is still holding it.
    let sd_core_module = Unmanaged<SDCoreModule>.fromOpaque(sd_core_module).takeUnretainedValue();
    
    // SAFTEY: We take as retained because the promise is consumed and should be deallocated in this call.
    let promise = Unmanaged<SwiftPromise>.fromOpaque(promise).takeRetainedValue();
    
    // An error occurred starting up the core **and** the error string was too long. This is an edge case and exists to avoid unwinding over FFI-boundary.
    if buf == nil {
        promise.p.reject(Exception(name: "BUF_TOO_LONG", description: "Rust was unable to return the result due to it being too long."));
        return;
    }
    
    let buf = UnsafeRawBufferPointer(start: buf, count: Int(len.pointee));
    let string = String(bytes: buf, encoding: String.Encoding.utf8);
    
    if status == 0 {
        promise.p.resolve(string);
    } else {
        promise.p.reject(Exception(name: "ERR", description: string ?? ""));
    }
}

// Used by Rust to take back control of a retained `SDCoreModule` so Swift can properly deallocate it.
//
// SAFTEY: Rust should assume the `sd_core_module` pointer is deallocated after this function so it must not continue to hold it!
@_cdecl("SDCoreModule_reretainSdCoreModule")
func reretain_sd_core_module(sd_core_module: UnsafeRawPointer) {
    _ = Unmanaged<SDCoreModule>.fromOpaque(sd_core_module).takeRetainedValue();
}

public class SDCoreModule: Module {
    // Rust `sd_mobile_core::State` struct.
    // It's heap allocated in Rust and we pass it to the core to do an operation on it.
    // We pass it back to Rust during shutdown and it's deallocated by Rust.
    // This will be `nil` if the core fails to statup.
    private var state: UnsafeRawPointer?;
    
    // The error string returned by the `sd_init_core` method to say whether it succeeded or not.
    fileprivate var coreStartupError: String?;
    
    // Each module class must implement the definition function. The definition consists of components
    // that describes the module's functionality and behavior.
    // See https://docs.expo.dev/modules/module-api for more details about available components.
    public func definition() -> ModuleDefinition {
        Name("SDCore")

        // Defines event that Rust uses to send messages back to JS.
        Events("sdCoreEvent")
        
        // A property to allow JS to determine if the core started up correctly
        Property("coreStartupError") {
            return self.coreStartupError;
        }

        // Initialse the Rust core when the application is started up.
        OnCreate {
            // TODO: Replace this whole block with `URL.documentsDirectory` once IOS 16 is our minimum supported version
            let documentsDirectory = try! FileManager.default.url(
              for: .documentDirectory,
              in: .userDomainMask,
              appropriateFor: nil,
              create: true).path;
            
            // SAFTEY: We use an unretained pointer as both arguments to `sd_init_core` have a lifetime of it's call so Rust should not hold onto them.
            let result = sd_init_core(UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque()), documentsDirectory);
            if let result {
                print("Core startup error: \(self.coreStartupError as Optional)");
                return;
            }
            
            // A new state is created for this instance of `SDCoreModule`.
            //
            // SAFETY: A retained reference means Swift will not drop this object while Rust is holding a reference to it.
            // SAFETY: In `OnDestroy` we wil ask Rust for the reference back so Swift is able to drop it.
            self.state = sd_init_state(UnsafeMutableRawPointer(Unmanaged.passRetained(self).toOpaque()));
        }
        
        // When requested to shutdown we ask Rust for the retained Swift reference to this class back so it can be properly deallocated by Swift.
        OnDestroy {
            if let state {
                sd_deinit_state(state);
                self.state = nil;
            }
        }

        // We are assuming that when all the listeners are removed, we are safe to remove all active subscriptions.
        // This should indicate a hot reload has taken place.
        OnStopObserving {
            if let state {
                sd_state_reset(state);
            }
        }
       
        // TODO
        AsyncFunction("sdCoreMsg") { (msg: String, promise: Promise) in
            if let state {
                let promise = UnsafeMutableRawPointer(Unmanaged.passRetained(SwiftPromise(promise: promise)).toOpaque());
                sd_core_msg(self.state, msg, promise);
            } else {
                promise.reject(Exception(name: "NO_STATE", description: "Core state is currently not initialised."))
            }
        }
    }
}
