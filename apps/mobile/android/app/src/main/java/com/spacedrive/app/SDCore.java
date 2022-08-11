package com.spacedrive.app;

import android.content.Context;
import android.os.Build;

import androidx.annotation.RequiresApi;

import com.facebook.react.bridge.Promise;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.bridge.ReactContextBaseJavaModule;
import com.facebook.react.bridge.ReactMethod;

public class SDCore extends ReactContextBaseJavaModule {
    SDCore(ReactApplicationContext context) { super(context); }

    @Override
    public String getName() {
        return "SDCore";
    }

    static {
        System.loadLibrary("sdcore");
    }

    private native void bruh(String query, Promise promise);

    @ReactMethod
    public void sd_core_msg(String query, Promise promise) {
        this.bruh(query, promise);
    }

    public String getDataDirectory() {
        return getCurrentActivity().getFilesDir().toString();
    }
}