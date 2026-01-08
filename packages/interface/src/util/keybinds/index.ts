// Types

export type { KeybindHandler } from "./listener";
// Listener
export {
  getWebListener,
  resetWebListener,
} from "./listener";

// Platform utilities
export {
  getComboForPlatform,
  getCurrentPlatform,
  isInputFocused,
  normalizeModifiers,
  toDisplayString,
  toTauriAccelerator,
} from "./platform";
export type { KeybindId } from "./registry";
// Registry
export {
  explorerKeybinds,
  getAllKeybinds,
  getKeybind,
  getKeybindsByScope,
  globalKeybinds,
  KEYBINDS,
  mediaViewerKeybinds,
  quickPreviewKeybinds,
} from "./registry";
export type {
  Key,
  KeybindDefinition,
  KeybindScope,
  KeyCombo,
  Modifier,
  Platform,
  PlatformKeyCombo,
} from "./types";
export { defineKeybind, isPlatformKeyCombo } from "./types";
