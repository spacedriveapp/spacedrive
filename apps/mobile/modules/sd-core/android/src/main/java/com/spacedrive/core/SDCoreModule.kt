package com.spacedrive.core

import expo.modules.kotlin.Promise
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import android.provider.Settings

class SDCoreModule : Module() {
	private var registeredWithRust = false
	private var listeners = 0

	init {
		System.loadLibrary("sd_mobile_android")
	}

	// is exposed by Rust and is used to register the subscription
	private external fun registerCoreEventListener()

	private external fun handleCoreMsg(query: String, promise: SDCorePromise)

	public fun getDataDirectory(): String {
        return appContext.persistentFilesDirectory.absolutePath;
    }

	 public fun printFromRust(msg: String) {
		print(msg);
	 }

	public fun sendCoreEvent(body: String) {
		if (listeners > 0) {
			this@SDCoreModule.sendEvent(
				"SDCoreEvent",
				mapOf(
					"body" to body
				)
			)
		}
	}

	public fun getDeviceName(): String {
		return Settings.Secure.getString(appContext.reactContext?.contentResolver, "device_name")
			?: "Android Spacedrive Device"
	}


	override fun definition() = ModuleDefinition {
		Name("SDCore")

		Events("SDCoreEvent")

		OnStartObserving {
			if (!registeredWithRust)
			{
				this@SDCoreModule.registerCoreEventListener();
			}

			this@SDCoreModule.listeners++;
		}

		OnStopObserving {
			this@SDCoreModule.listeners--;
		}

		AsyncFunction("sd_core_msg") { query: String, promise: Promise ->
			this@SDCoreModule.handleCoreMsg(query, SDCorePromise(promise))
		}

		Function("getDeviceName") {
			getDeviceName()
		}
	}
}
