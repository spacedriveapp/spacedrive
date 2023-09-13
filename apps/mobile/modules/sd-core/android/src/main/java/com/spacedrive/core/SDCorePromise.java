package com.spacedrive.core;

import expo.modules.kotlin.Promise;

public class SDCorePromise {
	public Promise promise;

	public SDCorePromise(Promise promise) {
		this.promise = promise;
	}

	public void resolve(String msg) {
		this.promise.resolve(msg);
	}
}
