import Foundation
import SwiftRs

@_cdecl("get_user_home_directory")
func getUserHomeDirectory() -> SRString {
    return SRString(FileManager.default.homeDirectoryForCurrentUser.path)
}
