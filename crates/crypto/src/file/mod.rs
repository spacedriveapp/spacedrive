//! This handles simple file encryption and decryption (easily accessed via the context menu)
use crate::primitives::BLOCK_SIZE;

pub mod encrypt;

// I'm unsure as to whether or not this will be needed, but I wrote it in advance just in case
// The target block will be related to the "step", e.g. step 200/4000
pub fn find_block_position(target_block: usize, header_size: usize) -> usize {    
    (target_block * BLOCK_SIZE) + header_size
}