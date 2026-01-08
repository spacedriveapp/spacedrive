import { type ClassValue, clsx } from "clsx";

/**
 * Utility function for combining class names.
 * Similar to clsx but optimized for NativeWind.
 */
export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}
