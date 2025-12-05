package com.spacedrive.core

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.Promise

class SDMobileCoreModule : Module() {
    private var listeners = 0
    private var registeredWithRust = false

    init {
        try {
            System.loadLibrary("sd_mobile_core")
        } catch (e: UnsatisfiedLinkError) {
            android.util.Log.e("SDMobileCore", "Failed to load native library: ${e.message}")
        }
    }

    override fun definition() = ModuleDefinition {
        Name("SDMobileCore")

        Events("SDCoreEvent")

        OnStartObserving {
            if (!registeredWithRust) {
                try {
                    registerCoreEventListener()
                    registeredWithRust = true
                } catch (e: Exception) {
                    android.util.Log.e("SDMobileCore", "Failed to register event listener: ${e.message}")
                }
            }
            listeners++
        }

        OnStopObserving {
            listeners--
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

    // Native methods - will throw UnsatisfiedLinkError if library not loaded
    private external fun registerCoreEventListener()
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
