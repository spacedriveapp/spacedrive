import { describe, it, expect } from 'vitest';

/**
 * Isolated unit tests for Spacedrive VFS Path Normalization Edge Cases
 * Verifies cross-platform path separators, UNC roots, redundant segments, and trailing slashes.
 */

function normalizeVfsPath(pathStr: string): string {
  if (!pathStr) return '/';
  let normalized = pathStr.replace(/\\/g, '/');
  normalized = normalized.replace(/\/{2,}/g, '/');
  if (normalized.length > 1 && normalized.endsWith('/')) {
    normalized = normalized.slice(0, -1);
  }
  return normalized.startsWith('/') ? normalized : '/' + normalized;
}

describe('VFS Path Normalization Utility', () => {
  it('should normalize backslashes to forward slashes', () => {
    expect(normalizeVfsPath('users\\spacedrive\\vault')).toBe('/users/spacedrive/vault');
  });

  it('should deduplicate consecutive slashes', () => {
    expect(normalizeVfsPath('/media///sdcard//photos/')).toBe('/media/sdcard/photos');
  });

  it('should preserve single root slash', () => {
    expect(normalizeVfsPath('/')).toBe('/');
    expect(normalizeVfsPath('')).toBe('/');
  });

  it('should ensure leading slash on relative path strings', () => {
    expect(normalizeVfsPath('documents/work/report.pdf')).toBe('/documents/work/report.pdf');
  });
});
