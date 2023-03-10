package com.spacedrive.app;

import android.content.Context;
import android.os.Build;

import androidx.annotation.RequiresApi;

import com.facebook.react.bridge.Promise;
import com.facebook.react.bridge.ReactApplicationContext;
import com.facebook.react.bridge.ReactContext;
import com.facebook.react.bridge.ReactContextBaseJavaModule;
import com.facebook.react.bridge.ReactMethod;
import com.facebook.react.bridge.WritableMap;
import com.facebook.react.modules.core.DeviceEventManagerModule;

import javax.annotation.Nullable;

public class SDCore extends ReactContextBaseJavaModule {
    SDCore(ReactApplicationContext context) { super(context); }

    private boolean registeredWithRust = false;
    private int listeners = 0;

    @Override
    public String getName()
    {
        return "SDCore";
    }

    static {
        System.loadLibrary("sd_mobile_android");
    }

    // is exposed by Rust and is used to register the subscription
    private native void registerCoreEventListener();

    private native void handleCoreMsg(String query, Promise promise);

    @ReactMethod
    public void sd_core_msg(String query, Promise promise)
    {
        this.handleCoreMsg(query, promise);
    }

    public String getDataDirectory()
    {
        return getCurrentActivity().getFilesDir().toString();
    }

    public void print(String msg)
    {
        System.out.println(msg);
    }

    @ReactMethod
    public void addListener(String eventName)
    {
        if (!registeredWithRust)
        {
            this.registerCoreEventListener();
        }

        this.listeners++;
    }

    @ReactMethod
    public void removeListeners(Integer count)
    {
        this.listeners--;
    }

    public void sendCoreEvent(String body)
    {
        if (this.listeners > 0)
        {
            this.getReactApplicationContext()
                    .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter.class)
                    .emit("SDCoreEvent", body);
        }
    }
}
