package com.spacedrive.core

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.Promise

class SDMobileCoreModule : Module() {
    private var listeners = 0
    private var logListeners = 0
    private var registeredWithRust = false
    private var logRegisteredWithRust = false

    init {
        try {
            System.loadLibrary("sd_mobile_core")
        } catch (e: UnsatisfiedLinkError) {
            android.util.Log.e("SDMobileCore", "Failed to load native library: ${e.message}")
        }
    }

    override fun definition() = ModuleDefinition {
        Name("SDMobileCore")

        Events("SDCoreEvent", "SDCoreLog")

        OnStartObserving("SDCoreEvent") {
            android.util.Log.i("SDMobileCore", "📡 OnStartObserving SDCoreEvent triggered")

            if (!registeredWithRust) {
                try {
                    android.util.Log.i("SDMobileCore", "🚀 Registering event listener...")
                    registerCoreEventListener()
                    registeredWithRust = true
                    android.util.Log.i("SDMobileCore", "✅ Event listener registered with Rust")
                } catch (e: Exception) {
                    android.util.Log.e("SDMobileCore", "Failed to register event listener: ${e.message}")
                }
            }

            listeners++
            android.util.Log.i("SDMobileCore", "📊 SDCoreEvent listeners: $listeners")
        }

        OnStopObserving("SDCoreEvent") {
            listeners--
            android.util.Log.i("SDMobileCore", "📉 SDCoreEvent listeners: $listeners")
        }

        OnStartObserving("SDCoreLog") {
            android.util.Log.i("SDMobileCore", "📡 OnStartObserving SDCoreLog triggered")

            if (!logRegisteredWithRust) {
                try {
                    android.util.Log.i("SDMobileCore", "🚀 Registering log listener...")
                    registerCoreLogListener()
                    logRegisteredWithRust = true
                    android.util.Log.i("SDMobileCore", "✅ Log listener registered with Rust")
                } catch (e: Exception) {
                    android.util.Log.e("SDMobileCore", "Failed to register log listener: ${e.message}")
                }
            }

            logListeners++
            android.util.Log.i("SDMobileCore", "📊 SDCoreLog listeners: $logListeners")
        }

        OnStopObserving("SDCoreLog") {
            logListeners--
            android.util.Log.i("SDMobileCore", "📉 SDCoreLog listeners: $logListeners")
        }

        Function("initialize") { dataDir: String?, deviceName: String? ->
            val dir = dataDir ?: appContext.persistentFilesDirectory?.absolutePath
                ?: throw Exception("No data directory available")

            try {
                initializeCore(dir, deviceName)
            } catch (e: Exception) {
                android.util.Log.e("SDMobileCore", "Failed to initialize core: ${e.message}")
                -1
            }
        }

        AsyncFunction("sendMessage") { query: String, promise: Promise ->
            try {
                handleCoreMsg(query, SDCorePromise(promise))
            } catch (e: Exception) {
                promise.reject("CORE_ERROR", e.message ?: "Unknown error", e)
            }
        }

        Function("shutdown") {
            try {
                shutdownCore()
            } catch (e: Exception) {
                android.util.Log.e("SDMobileCore", "Failed to shutdown core: ${e.message}")
            }
        }
    }

    fun getDataDirectory(): String {
        return appContext.persistentFilesDirectory?.absolutePath ?: ""
    }

    fun sendCoreEvent(body: String) {
        if (listeners > 0) {
            this@SDMobileCoreModule.sendEvent("SDCoreEvent", mapOf("body" to body))
        }
    }

    fun sendCoreLog(body: String) {
        if (logListeners > 0) {
            this@SDMobileCoreModule.sendEvent("SDCoreLog", mapOf("body" to body))
        }
    }

    // Native methods - will throw UnsatisfiedLinkError if library not loaded
    private external fun registerCoreEventListener()
    private external fun registerCoreLogListener()
    private external fun initializeCore(dataDir: String, deviceName: String?): Int
    private external fun handleCoreMsg(query: String, promise: SDCorePromise)
    private external fun shutdownCore()
}

class SDCorePromise(private val promise: Promise) {
    fun resolve(msg: String) {
        promise.resolve(msg)
    }

    fun reject(error: String) {
        promise.reject("CORE_ERROR", error, null)
    }
}
