package com.spacedrive.core

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.os.storage.StorageManager
import android.provider.DocumentsContract
import android.util.Log
import androidx.core.content.ContextCompat
import androidx.documentfile.provider.DocumentFile
import expo.modules.kotlin.Promise
import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.records.Field
import expo.modules.kotlin.records.Record
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

// Options class for folder picker (required for AsyncFunction pattern with activity results)
class FolderPickerOptions : Record {
	@Field
	val dummy: String? = null
}

// Function count exceeds threshold due to small, focused helper functions
// extracted from complex SAF path resolution logic - this is preferred over
// having fewer but more complex functions.
@Suppress("TooManyFunctions")
class SDMobileCoreModule : Module() {
	// Thread-safe listener counters
	private val listeners = AtomicInteger(0)
	private val logListeners = AtomicInteger(0)
	private val registeredWithRust = AtomicBoolean(false)
	private val logRegisteredWithRust = AtomicBoolean(false)

	// Thread-safe promise storage for concurrent folder picker calls
	// Maps request code to pending promise
	private val pendingFolderPickerPromises = ConcurrentHashMap<Int, Promise>()
	private val requestCodeCounter = AtomicInteger(FOLDER_PICKER_REQUEST_CODE_BASE)

	companion object {
		private const val FOLDER_PICKER_REQUEST_CODE_BASE = 9999
		private const val TAG = "SDMobileCore"

		// Cached debug state - set during initialization
		@Volatile
		private var isDebugBuild: Boolean = true // Default to debug for safety

		/**
		 * Initialize the debug state based on application flags.
		 * Should be called once during module initialization.
		 */
		fun initDebugState(context: Context?) {
			context?.applicationInfo?.let { appInfo ->
				isDebugBuild = appInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0
			}
		}

		/**
		 * Log a debug message only in debug builds.
		 * In release builds, this is a no-op.
		 */
		fun debugLog(message: String) {
			if (isDebugBuild) {
				Log.d(TAG, message)
			}
		}

		/**
		 * Sanitize a path for logging to avoid exposing user directory structure.
		 * In debug builds, returns the full path for easier debugging.
		 * In release builds, returns only the last path component.
		 */
		fun sanitizePath(path: String?): String {
			if (path == null) return "<null>"
			return if (isDebugBuild) {
				path
			} else {
				// Only show last path component in release
				val lastSeparator = path.lastIndexOf('/')
				if (lastSeparator >= 0 && lastSeparator < path.length - 1) {
					".../${path.substring(lastSeparator + 1)}"
				} else {
					"..."
				}
			}
		}

		/**
		 * Check if the app has appropriate storage permissions for the current Android version.
		 * - Android 11+ (API 30+): Checks MANAGE_EXTERNAL_STORAGE
		 * - Android 10 (API 29): Checks READ_EXTERNAL_STORAGE
		 * - Android 9 and below: Checks READ_EXTERNAL_STORAGE
		 *
		 * @return true if the app has sufficient storage permissions
		 */
		fun hasStoragePermission(context: Context): Boolean {
			return when {
				Build.VERSION.SDK_INT >= Build.VERSION_CODES.R -> {
					// Android 11+ - check for MANAGE_EXTERNAL_STORAGE
					Environment.isExternalStorageManager()
				}
				Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q -> {
					// Android 10 - scoped storage, but can still check basic permission
					ContextCompat.checkSelfPermission(
						context,
						Manifest.permission.READ_EXTERNAL_STORAGE,
					) == PackageManager.PERMISSION_GRANTED
				}
				else -> {
					// Android 9 and below
					ContextCompat.checkSelfPermission(
						context,
						Manifest.permission.READ_EXTERNAL_STORAGE,
					) == PackageManager.PERMISSION_GRANTED
				}
			}
		}

		/**
		 * Log a warning if storage permission is not granted.
		 * This helps developers understand why file operations might fail.
		 */
		fun warnIfNoStoragePermission(
			context: Context?,
			operation: String,
		) {
			if (context == null) return
			if (!hasStoragePermission(context)) {
				Log.w(
					TAG,
					"Storage permission not granted for operation: $operation. " +
						"File access may fail or be limited to app-specific directories.",
				)
			}
		}

		/**
		 * Validate and resolve a path, checking for path traversal attacks.
		 *
		 * Security checks performed:
		 * 1. Reject null or empty paths
		 * 2. Split path into components and check each one
		 * 3. Reject paths with ".." components (parent traversal)
		 * 4. Reject paths starting with "/" (absolute paths) when relative expected
		 * 5. Resolve canonical path and verify it's under expected base
		 *
		 * @param basePath The allowed base directory
		 * @param relativePath The relative path to validate
		 * @return The validated canonical path, or null if validation fails
		 */
		fun validateAndResolvePath(
			basePath: String,
			relativePath: String,
		): String? {
			// Reject empty paths
			if (relativePath.isBlank()) {
				return basePath
			}

			// Split and validate each path component
			val components = relativePath.split("/").filter { it.isNotEmpty() }
			for (component in components) {
				// Reject parent directory traversal
				if (component == "..") {
					Log.w(TAG, "Path traversal attempt detected: contains '..'")
					return null
				}
				// Reject hidden directories/files starting with . (optional, stricter)
				// component.startsWith(".") could be added here if needed
			}

			// Construct the full path
			val fullPath =
				if (relativePath.isNotEmpty()) {
					"$basePath/$relativePath"
				} else {
					basePath
				}

			// Resolve to canonical path and verify it's still under base
			return try {
				val baseFile = java.io.File(basePath).canonicalFile
				val targetFile = java.io.File(fullPath).canonicalFile

				// Check that the canonical path is under the base path
				// This catches symlink-based escape attempts
				if (!targetFile.absolutePath.startsWith(baseFile.absolutePath)) {
					Log.w(TAG, "Path escape attempt: resolved path is outside base directory")
					null
				} else {
					targetFile.absolutePath
				}
			} catch (e: Exception) {
				Log.w(TAG, "Path validation failed: ${e.message}")
				null
			}
		}
	}

	init {
		try {
			System.loadLibrary("sd_mobile_core")
		} catch (e: UnsatisfiedLinkError) {
			Log.e(TAG, "Failed to load native library: ${e.message}")
		}
	}

	// Expo module API requires all event handlers and functions to be declared
	// within the definition block, making this inherently long.
	@Suppress("LongMethod", "CyclomaticComplexMethod")
	override fun definition() =
		ModuleDefinition {
			Name("SDMobileCore")

			// Initialize debug state based on app's debuggable flag
			initDebugState(appContext.reactContext)

			Events("SDCoreEvent", "SDCoreLog")

			OnStartObserving("SDCoreEvent") {
				Log.i(TAG, "OnStartObserving SDCoreEvent triggered")

				if (registeredWithRust.compareAndSet(false, true)) {
					try {
						Log.i(TAG, "Registering event listener...")
						registerCoreEventListener()
						Log.i(TAG, "Event listener registered with Rust")
					} catch (e: Exception) {
						registeredWithRust.set(false) // Reset on failure
						Log.e(TAG, "Failed to register event listener: ${e.message}")
					}
				}

				val count = listeners.incrementAndGet()
				Log.i(TAG, "SDCoreEvent listeners: $count")
			}

			OnStopObserving("SDCoreEvent") {
				val count = listeners.decrementAndGet()
				Log.i(TAG, "SDCoreEvent listeners: $count")
			}

			OnStartObserving("SDCoreLog") {
				Log.i(TAG, "OnStartObserving SDCoreLog triggered")

				if (logRegisteredWithRust.compareAndSet(false, true)) {
					try {
						Log.i(TAG, "Registering log listener...")
						registerCoreLogListener()
						Log.i(TAG, "Log listener registered with Rust")
					} catch (e: Exception) {
						logRegisteredWithRust.set(false) // Reset on failure
						Log.e(TAG, "Failed to register log listener: ${e.message}")
					}
				}

				val count = logListeners.incrementAndGet()
				Log.i(TAG, "SDCoreLog listeners: $count")
			}

			OnStopObserving("SDCoreLog") {
				val count = logListeners.decrementAndGet()
				Log.i(TAG, "SDCoreLog listeners: $count")
			}

			Function("initialize") { dataDir: String?, deviceName: String? ->
				val dir =
					dataDir ?: appContext.persistentFilesDirectory?.absolutePath
						?: throw Exception("No data directory available")

				try {
					initializeCore(dir, deviceName)
				} catch (e: Exception) {
					Log.e(TAG, "Failed to initialize core: ${e.message}")
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
					Log.e(TAG, "Failed to shutdown core: ${e.message}")
				}
			}

			// Simple test function
			Function("testFunction") {
				Log.i(TAG, "testFunction called!")
				"test_result"
			}

			// Open Android folder picker using Storage Access Framework
			// TODO: Migrate to ActivityResultContracts when Expo modules support it
			// See: https://github.com/expo/expo/issues/TBD
			@Suppress("DEPRECATION")
			AsyncFunction("pickFolder") { options: FolderPickerOptions, promise: Promise ->
				val activity = appContext.currentActivity
				if (activity == null) {
					promise.reject(CodedException("NO_ACTIVITY", "No activity available", null))
					return@AsyncFunction
				}

				try {
					val intent =
						Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
							addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
							addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
							addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
							addFlags(Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
						}

					// Generate unique request code for concurrent calls
					val requestCode = requestCodeCounter.incrementAndGet()
					pendingFolderPickerPromises[requestCode] = promise

					@Suppress("DEPRECATION")
					activity.startActivityForResult(intent, requestCode)
				} catch (e: Exception) {
					Log.e(TAG, "Failed to open folder picker: ${e.message}")
					promise.reject(CodedException("PICKER_ERROR", e.message ?: "Failed to open folder picker", e))
				}
			}

			// Get the real filesystem path from a content URI (if possible)
			Function("getPathFromUri") { uriString: String ->
				try {
					val uri = Uri.parse(uriString)
					getPathFromContentUri(uri)
				} catch (e: Exception) {
					Log.e(TAG, "Failed to get path from URI: ${e.message}")
					null
				}
			}

			// Check if the app has full storage access permission (Android 11+)
			Function("hasStoragePermission") {
				val context = appContext.reactContext ?: return@Function false
				hasStoragePermission(context)
			}

			// Check if full storage permission is required (Android 11+)
			Function("requiresStoragePermission") {
				Build.VERSION.SDK_INT >= Build.VERSION_CODES.R
			}

			// Open the system settings page to grant "All Files Access" permission
			Function("openStoragePermissionSettings") {
				val context = appContext.reactContext ?: return@Function false
				try {
					val action = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
						android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION
					} else {
						android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS
					}
					val intent = Intent(action, Uri.parse("package:${context.packageName}")).apply {
						addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
					}
					context.startActivity(intent)
					true
				} catch (e: Exception) {
					Log.e(TAG, "Failed to open storage permission settings: ${e.message}")
					false
				}
			}

			OnActivityResult { _, payload ->
				// Look up promise by request code from concurrent-safe map
				val promise = pendingFolderPickerPromises.remove(payload.requestCode)

				if (promise == null) {
					// Not our request code, ignore
					return@OnActivityResult
				}

				if (payload.resultCode != Activity.RESULT_OK) {
					promise.reject(CodedException("CANCELLED", "Folder picker was cancelled", null))
					return@OnActivityResult
				}

				val uri = payload.data?.data
				if (uri == null) {
					promise.reject(CodedException("NO_URI", "No folder URI returned", null))
					return@OnActivityResult
				}

				// Take persistent permissions
				try {
					val takeFlags =
						Intent.FLAG_GRANT_READ_URI_PERMISSION or
							Intent.FLAG_GRANT_WRITE_URI_PERMISSION
					appContext.reactContext?.contentResolver?.takePersistableUriPermission(uri, takeFlags)
				} catch (e: SecurityException) {
					Log.w(TAG, "Failed to take persistent permission (SecurityException): ${e.message}")
				} catch (e: IllegalArgumentException) {
					Log.w(TAG, "Failed to take persistent permission (invalid URI): ${e.message}")
				}

				// Check storage permissions before path resolution
				warnIfNoStoragePermission(appContext.reactContext, "folder picker path resolution")

				// Try to get the real path
				val realPath = getPathFromContentUri(uri)
				val folderName =
					appContext.reactContext?.let { context ->
						DocumentFile.fromTreeUri(context, uri)?.name
					} ?: "Unknown"

				val result =
					mapOf(
						"uri" to uri.toString(),
						"path" to realPath,
						"name" to folderName,
					)

				promise.resolve(result)
			}
		}

	fun getDataDirectory(): String {
		return appContext.persistentFilesDirectory?.absolutePath ?: ""
	}

	fun sendCoreEvent(body: String) {
		if (listeners.get() > 0) {
			this@SDMobileCoreModule.sendEvent("SDCoreEvent", mapOf("body" to body))
		}
	}

	fun sendCoreLog(body: String) {
		if (logListeners.get() > 0) {
			this@SDMobileCoreModule.sendEvent("SDCoreLog", mapOf("body" to body))
		}
	}

	/**
	 * Attempts to convert a content:// URI to a real filesystem path.
	 * This works for primary external storage on most devices.
	 */
	private fun getPathFromContentUri(uri: Uri): String? {
		// Handle document tree URIs (from ACTION_OPEN_DOCUMENT_TREE)
		if (DocumentsContract.isTreeUri(uri)) {
			val docId = DocumentsContract.getTreeDocumentId(uri)
			return getPathFromDocId(docId)
		}

		// Handle regular document URIs
		if (DocumentsContract.isDocumentUri(appContext.reactContext, uri)) {
			val docId = DocumentsContract.getDocumentId(uri)
			return getPathFromDocId(docId)
		}

		return null
	}

	/**
	 * Parse and validate a document ID, returning the storage ID and relative path.
	 * Returns null if the document ID is invalid or contains path traversal attempts.
	 */
	private fun parseDocumentId(docId: String): Pair<String, String>? {
		if (docId.isBlank()) {
			Log.w(TAG, "Empty document ID provided")
			return null
		}

		// Document ID format: "primary:path/to/folder" or "storageId:path/to/folder"
		val split = docId.split(":", limit = 2)
		if (split.size < 2) {
			Log.w(TAG, "Invalid document ID format: $docId")
			return null
		}

		val storageId = split[0]
		val relativePath = split[1]

		if (!isRelativePathSafe(relativePath)) {
			return null
		}

		return Pair(storageId, relativePath)
	}

	/**
	 * Check if a relative path is safe (no traversal attempts, not absolute).
	 */
	private fun isRelativePathSafe(relativePath: String): Boolean {
		if (relativePath.startsWith("/")) {
			Log.w(TAG, "Absolute path in document ID rejected: $relativePath")
			return false
		}

		val pathComponents = relativePath.split("/").filter { it.isNotEmpty() }
		for (component in pathComponents) {
			if (component == "..") {
				Log.w(TAG, "Path traversal attempt detected: $relativePath")
				return false
			}
			if (component == ".") {
				Log.w(TAG, "Suspicious path component '.' in: $relativePath")
				return false
			}
		}
		return true
	}

	/**
	 * Resolve a path for secondary storage volumes using fallback mount points.
	 */
	private fun resolveViaFallbackMounts(
		storageId: String,
		relativePath: String,
	): String? {
		val possibleBases =
			listOf(
				"/storage/$storageId",
				"/mnt/media_rw/$storageId",
				"/mnt/usb/$storageId",
			)

		return possibleBases
			.filter { java.io.File(it).exists() }
			.mapNotNull { validateAndResolvePath(it, relativePath) }
			.firstOrNull()
	}

	private fun getPathFromDocId(docId: String): String? {
		val (storageId, relativePath) = parseDocumentId(docId) ?: return null

		return when (storageId) {
			"primary" -> resolvePrimaryStorage(relativePath)
			"home" -> resolveHomeStorage(relativePath)
			else -> resolveSecondaryStorage(storageId, relativePath)
		}
	}

	@Suppress("DEPRECATION")
	private fun resolvePrimaryStorage(relativePath: String): String? {
		val basePath = Environment.getExternalStorageDirectory().absolutePath
		return validateAndResolvePath(basePath, relativePath)
	}

	@Suppress("DEPRECATION")
	private fun resolveHomeStorage(relativePath: String): String? {
		val documentsDir =
			Environment.getExternalStoragePublicDirectory(
				Environment.DIRECTORY_DOCUMENTS,
			)
		return validateAndResolvePath(documentsDir.absolutePath, relativePath)
	}

	private fun resolveSecondaryStorage(
		storageId: String,
		relativePath: String,
	): String? {
		// Try StorageManager API first on Android N+
		tryResolveViaStorageManager(storageId, relativePath)?.let { return it }

		// Fallback: Try common mount points
		val result = resolveViaFallbackMounts(storageId, relativePath)
		if (result == null) {
			Log.w(TAG, "Could not resolve path for storage ID: $storageId")
		}
		return result
	}

	/**
	 * Try to resolve a storage volume path using StorageManager API (Android N+).
	 * This provides more reliable path resolution than hardcoded mount points.
	 */
	private fun tryResolveViaStorageManager(
		storageId: String,
		relativePath: String,
	): String? {
		if (Build.VERSION.SDK_INT < Build.VERSION_CODES.N) return null

		val context = appContext.reactContext ?: return null
		val storageManager =
			context.getSystemService(Context.STORAGE_SERVICE) as? StorageManager
				?: return null

		return try {
			@Suppress("DEPRECATION")
			storageManager.storageVolumes
				.firstOrNull { it.uuid?.equals(storageId, ignoreCase = true) == true }
				?.let { volume -> resolveVolumeToPath(volume, storageId, relativePath) }
		} catch (e: Exception) {
			Log.w(TAG, "StorageManager resolution failed: ${e.message}")
			null
		}
	}

	/**
	 * Resolve a matched StorageVolume to a filesystem path.
	 */
	private fun resolveVolumeToPath(
		volume: android.os.storage.StorageVolume,
		storageId: String,
		relativePath: String,
	): String? {
		// On Android R+, we can get the directory directly
		if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
			volume.directory?.absolutePath?.let { basePath ->
				return buildPath(basePath, relativePath)
			}
		}

		// Fallback: construct path from common patterns
		val possibleBase = "/storage/$storageId"
		if (java.io.File(possibleBase).exists()) {
			return buildPath(possibleBase, relativePath)
		}

		return null
	}

	private fun buildPath(
		basePath: String,
		relativePath: String,
	): String {
		return if (relativePath.isNotEmpty()) "$basePath/$relativePath" else basePath
	}

	// Native methods - will throw UnsatisfiedLinkError if library not loaded
	private external fun registerCoreEventListener()

	private external fun registerCoreLogListener()

	private external fun initializeCore(
		dataDir: String,
		deviceName: String?,
	): Int

	private external fun handleCoreMsg(
		query: String,
		promise: SDCorePromise,
	)

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
