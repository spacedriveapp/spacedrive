//
//  SDCore.m
//  Spacedrive
//
//  Created by Oscar Beaumont on 24/7/2023.
//

// NSString* helloFromObjC() {
//     return @"Hello From Objective C Using Expo Native Modules!";
// }

// is a function defined in Rust which starts a listener for Rust events.
// void register_core_event_listener(id objc_class);

// is a function defined in Rust which is responsible for handling messages from the frontend.
// void sd_core_msg(uint32_t context_id, const char* query, void* resolve, void* reject);

// is a function defined in Rust which is responsible for setting all rspc subscriptions.
// void sd_cleanup_context(uint32_t context_id);

// is called by Rust to determine the base directory to store data in. This is only done when initialising the Node.
// const char* get_data_directory(void)
// {
//   NSArray *dirPaths = dirPaths = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,
//                                                                      NSUserDomainMask, YES);
//   const char *docDir = [ [dirPaths objectAtIndex:0] UTF8String];
//   return docDir;
// }

// is called by Rust with a void* to the resolve function (of type RCTPromiseResolveBlock) to call it.
// Due to 'RCTPromiseResolveBlock' being an Objective-C block it is hard to call from Rust.
// void call_resolve(void *resolvePtr, const char* resultRaw)
// {
//   RCTPromiseResolveBlock resolve = (__bridge RCTPromiseResolveBlock) resolvePtr;
//   NSString *result = [NSString stringWithUTF8String:resultRaw];
//   resolve(result);
//   [result release];
// }

// is called by Rust with a void* to the resolve function (of type RCTPromiseRejectBlock) to call it.
// Due to 'RCTPromiseRejectBlock' being an Objective-C block it is hard to call from Rust.
// void call_reject(void *rejectPtr, const char* resultRaw)
// {
//   RCTPromiseRejectBlock reject = (__bridge RCTPromiseRejectBlock) rejectPtr;
//   NSString *result = [NSString stringWithUTF8String:resultRaw];
//   reject(@"event_failure", result, nil);
//   [result release];
// }
