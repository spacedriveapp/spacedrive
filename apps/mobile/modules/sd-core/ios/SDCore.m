//
//  SDCore.m
//  Spacedrive
//
//  TODO: At one point this was a requirement. No idea if it's still correct.
//  This file will not work unless ARC is disabled. Do this by setting the compiler flag '-fno-objc-arc' on this file in Settings > Build Phases > Compile Sources.
//
//  Created by Oscar Beaumont on 24/7/2023.
//

#include "SDCore.h"

// TODO: Move to Swift
// is called by Rust to determine the base directory to store data in. This is only done when initialising the Node.
const char* get_data_directory(void)
{
 NSArray *dirPaths = dirPaths = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,
                                                                    NSUserDomainMask, YES);
 const char *docDir = [ [dirPaths objectAtIndex:0] UTF8String];
 return docDir;
}
