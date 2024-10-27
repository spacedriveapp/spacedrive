import SwiftRs
import UIKit

@_cdecl("fetch_device_name")
func fetchDeviceName() -> SRString {
		// If we obtain the proper entitlement, this will return the device name -- otherwise,
		// on iOS 16.0 or newer, it'll return the device model. -iLynxcat 26/oct/2024 
		// See: https://developer.apple.com/documentation/uikit/uidevice/1620015-name#discussion
		return SRString(UIDevice.current.name)
}
