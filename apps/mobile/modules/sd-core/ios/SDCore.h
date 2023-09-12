//
//  SDCore.h
//  Spacedrive
//
//  This file is a header file for the functions defined in Rust and exposed using the C ABI.
//  You must ensure it matches the implementations within the Rust crate.
//
//  Created by Oscar Beaumont on 24/7/2023.
//

#ifndef SDCore_h
#define SDCore_h

// FUNCTIONS DEFINED IN RUST

// is a function defined in Rust which starts a listener for Rust events.
void register_core_event_listener(const void *module);

// is a function defined in Rust which is responsible for handling messages from the frontend.
void sd_core_msg(const char *query, const void *resolve);

#endif /* SDCore_h */
