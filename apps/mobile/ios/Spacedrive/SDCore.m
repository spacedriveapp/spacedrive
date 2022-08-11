//
//  SDCore.m
//  Spacedrive
//
//  This file will not work unless ARC is disabled. Do this by setting the compiler flag '-fno-objc-arc' on this file in Settings > Build Phases > Compile Sources.
//  This file also expects the Spacedrive Rust library to be linked in Settings > Build Phases > Link Binary with Libraries.
//
//  Created by Oscar Beaumont on 9/8/2022.
//

#import "SDCore.h"
#import <React/RCTLog.h>

// TODO
void register_node(id objc_class);

// is a function defined in Rust which is responsible for handling messages from the frontend.
void sd_core_msg(const char* query, void* resolve);

// is called by Rust to determine the base directory to store data in. This is only done when initialising the Node.
const char* get_data_directory(void)
{
  NSArray *dirPaths = dirPaths = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,
                                                                     NSUserDomainMask, YES);
  const char *docDir = [ [dirPaths objectAtIndex:0] UTF8String];
  return docDir;
}

// is called by Rust with a void* to the resolve function (of type RCTPromiseResolveBlock) to call it.
// Due to 'RCTPromiseResolveBlock' being an Objective-C block it is hard to call from Rust.
void call_resolve(void *resolvePtr, const char* resultRaw)
{
  RCTPromiseResolveBlock resolve = (__bridge RCTPromiseResolveBlock) resolvePtr;
  NSString *result = [NSString stringWithUTF8String:resultRaw];
  resolve(result);
  [result release];
}

@implementation SDCore

- (instancetype)init {
    if (self = [super init]) {
      id this = self;
      register_node(this);
    }
    return self;
}

RCT_EXPORT_MODULE();

RCT_EXPORT_METHOD(sd_core_msg: (NSString *)queryRaw
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)
{
  const char *query = [queryRaw UTF8String];
  sd_core_msg(query, (__bridge void*) [resolve retain]);
}

- (NSArray<NSString *> *)supportedEvents {
  return @[@"sayHello"];
}

- (void)tellJS {
  RCTLogInfo(@"Here");
  [self sendEventWithName:@"sayHello" body:@"Hello"];
}


@end
