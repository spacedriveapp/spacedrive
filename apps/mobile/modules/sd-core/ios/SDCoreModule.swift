import ExpoModulesCore

// A class wrapper around the `Promise` value type.
private class SwiftPromise {
    var p: Promise;

    init(promise: Promise) {
        self.p = promise;
    }
}

// is called by Rust to resolve a Promise with some data.
@_cdecl("sd_core_event")
func sd_core_event(this: UnsafeRawPointer, data: UnsafePointer<CChar>) {
    // The pointer is retained but we take it unretained because it's ownership is not being gived back to Swift permanently.
    // The pointer *will* be reused and dropping it now would be UB.
    let this = Unmanaged<SDCoreModule>.fromOpaque(this).takeUnretainedValue();
    if (this.listeners > 0) {
        this.sendEvent("SDCoreEvent", [
            "body": String(cString: data)
        ])
    }
}

// is called by Rust to resolve a Promise with some data.
@_cdecl("call_resolve")
func call_resolve(this: UnsafeRawPointer, data: UnsafePointer<CChar>) {
    let promise = Unmanaged<SwiftPromise>.fromOpaque(this).takeRetainedValue();
    promise.p.resolve(String(cString: data));
}

public class SDCoreModule: Module {
    var registeredWithRust = false;
    var listeners = 0;

    public func definition() -> ModuleDefinition {
        Name("SDCore")

        Events("SDCoreEvent")

        OnStartObserving {
            if (!registeredWithRust)
            {
                // TODO: Passing it as retained isn't great because it means this class will never be destroyed.
                // That being said if it isn't retained it would be very unsafe with the current Rust code so it needs to be improved first.
                register_core_event_listener(UnsafeRawPointer(Unmanaged.passRetained(self).toOpaque()));
                registeredWithRust = true;
            }

            listeners += 1;
        }

        OnStopObserving {
            listeners -= 1;
        }

        AsyncFunction("sd_core_msg") { (query: String, promise: Promise) in
            let promise = UnsafeMutableRawPointer(Unmanaged.passRetained(SwiftPromise(promise: promise)).toOpaque());
            sd_core_msg((query as NSString).utf8String, promise);
        }
    }
}
