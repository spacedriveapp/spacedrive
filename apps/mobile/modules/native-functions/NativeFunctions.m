//
//  NativeFunctions.m
//  Spacedrive
//
//  Created by Arnab Chakraborty on November 27, 2024.
//

#import <Foundation/Foundation.h>
#import <React/RCTBridgeModule.h>

@interface RCT_EXTERN_MODULE(NativeFunctions, NSObject)

RCT_EXTERN_METHOD(saveLocation:(nonnull NSString *)path
                  locationId:(nonnull NSNumber *)locationId
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

RCT_EXTERN_METHOD(previewFile:(nonnull NSString *)path
                  locationId:(nonnull NSNumber *)locationId
                  resolver:(RCTPromiseResolveBlock)resolve
                  rejecter:(RCTPromiseRejectBlock)reject)

@end

