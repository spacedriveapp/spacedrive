//! Magic byte pattern matching

use serde::{Deserialize, Serialize};
use std::fmt;

/// A pattern of magic bytes for file identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicBytePattern {
    /// The byte pattern
    pub bytes: Vec<MagicByte>,
    
    /// Offset from start of file
    pub offset: usize,
    
    /// Priority for conflict resolution (higher = more specific)
    pub priority: u8,
}

/// A single byte in a magic pattern
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MagicByte {
    /// Exact byte value
    Exact(u8),
    
    /// Any byte (wildcard)
    Any,
    
    /// Range of values
    Range { min: u8, max: u8 },
}

impl MagicBytePattern {
    /// Create a pattern from hex string (e.g., "FF D8 FF ?? 00-FF")
    pub fn from_hex_string(s: &str, offset: usize, priority: u8) -> Result<Self, String> {
        let bytes = s
            .split_whitespace()
            .map(|part| {
                if part == "??" || part == "?" {
                    Ok(MagicByte::Any)
                } else if part.contains('-') {
                    let parts: Vec<&str> = part.split('-').collect();
                    if parts.len() != 2 {
                        return Err(format!("Invalid range: {}", part));
                    }
                    let min = u8::from_str_radix(parts[0], 16)
                        .map_err(|_| format!("Invalid hex: {}", parts[0]))?;
                    let max = u8::from_str_radix(parts[1], 16)
                        .map_err(|_| format!("Invalid hex: {}", parts[1]))?;
                    Ok(MagicByte::Range { min, max })
                } else {
                    u8::from_str_radix(part, 16)
                        .map(MagicByte::Exact)
                        .map_err(|_| format!("Invalid hex: {}", part))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(Self {
            bytes,
            offset,
            priority,
        })
    }
    
    /// Check if this pattern matches the given buffer
    pub fn matches(&self, buf: &[u8]) -> bool {
        let start = self.offset;
        let end = start + self.bytes.len();
        
        if buf.len() < end {
            return false;
        }
        
        let slice = &buf[start..end];
        
        for (i, byte_pattern) in self.bytes.iter().enumerate() {
            if !byte_pattern.matches(slice[i]) {
                return false;
            }
        }
        
        true
    }
    
    /// Get the minimum buffer size needed to check this pattern
    pub fn required_size(&self) -> usize {
        self.offset + self.bytes.len()
    }
}

impl MagicByte {
    /// Check if this pattern matches a byte
    pub fn matches(&self, byte: u8) -> bool {
        match self {
            MagicByte::Exact(b) => *b == byte,
            MagicByte::Any => true,
            MagicByte::Range { min, max } => byte >= *min && byte <= *max,
        }
    }
}

impl fmt::Display for MagicByte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MagicByte::Exact(b) => write!(f, "{:02X}", b),
            MagicByte::Any => write!(f, "??"),
            MagicByte::Range { min, max } => write!(f, "{:02X}-{:02X}", min, max),
        }
    }
}

impl fmt::Display for MagicBytePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "offset={}: ", self.offset)?;
        for (i, byte) in self.bytes.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", byte)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_magic_byte_pattern_from_hex() {
        let pattern = MagicBytePattern::from_hex_string("FF D8 FF", 0, 100).unwrap();
        assert_eq!(pattern.bytes.len(), 3);
        assert!(matches!(pattern.bytes[0], MagicByte::Exact(0xFF)));
        assert!(matches!(pattern.bytes[1], MagicByte::Exact(0xD8)));
        assert!(matches!(pattern.bytes[2], MagicByte::Exact(0xFF)));
    }
    
    #[test]
    fn test_magic_byte_pattern_with_wildcards() {
        let pattern = MagicBytePattern::from_hex_string("47 ?? ?? 47", 0, 90).unwrap();
        assert_eq!(pattern.bytes.len(), 4);
        assert!(matches!(pattern.bytes[0], MagicByte::Exact(0x47)));
        assert!(matches!(pattern.bytes[1], MagicByte::Any));
        assert!(matches!(pattern.bytes[2], MagicByte::Any));
        assert!(matches!(pattern.bytes[3], MagicByte::Exact(0x47)));
    }
    
    #[test]
    fn test_pattern_matching() {
        let pattern = MagicBytePattern::from_hex_string("FF D8", 0, 100).unwrap();
        assert!(pattern.matches(&[0xFF, 0xD8, 0xFF]));
        assert!(!pattern.matches(&[0xFF, 0xD7]));
        assert!(!pattern.matches(&[0xFF])); // Too short
        
        // Test with offset
        let pattern = MagicBytePattern::from_hex_string("50 4B", 2, 100).unwrap();
        assert!(pattern.matches(&[0x00, 0x00, 0x50, 0x4B]));
        assert!(!pattern.matches(&[0x50, 0x4B, 0x00, 0x00]));
    }
}