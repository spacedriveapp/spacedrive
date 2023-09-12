import ExpoModulesCore

public class SDCoreModule: Module {
  public func definition() -> ModuleDefinition {
    Name("SDCore")

    // TODO: The `Podspec` in this module currently deals with building and linking the Rust core but the core messages sill go through the bridge code in the main IOS project (`SDCore.m`).
    // TODO: I couldn't get Objective-C working in an Expo Native Module last time I tried and porting the code is not outta scope for this PR as IOS is actually working fine.
  }
}
