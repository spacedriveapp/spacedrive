import { keybindForOs } from '~/util/keybinds';
import { useOperatingSystem } from './useOperatingSystem';

export const useKeybindFactory = () => keybindForOs(useOperatingSystem());
