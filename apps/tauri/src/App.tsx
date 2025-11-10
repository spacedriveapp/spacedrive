import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Explorer, FloatingControls, LocationCacheDemo, Inspector, QuickPreview, PlatformProvider } from "@sd/interface";
import { SpacedriveClient, TauriTransport } from "@sd/ts-client";
import { useEffect, useState } from "react";
import { DragOverlay } from "./routes/DragOverlay";
import { DragDemo } from "./components/DragDemo";
import { platform } from "./platform";

function App() {
  console.log("App component rendering");

  const [client, setClient] = useState<SpacedriveClient | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [route, setRoute] = useState<string>("/");

  useEffect(() => {
    console.log("Tauri App mounting...");

    // Get current window label to determine which UI to show
    const currentWindow = getCurrentWebviewWindow();
    const label = currentWindow.label;
    console.log("Window label:", label);

    // Set route based on window label
    if (label === "floating-controls") {
      setRoute("/floating-controls");
    } else if (label.startsWith("drag-overlay")) {
      setRoute("/drag-overlay");
    } else if (label.startsWith("drag-demo")) {
      setRoute("/drag-demo");
    } else if (label.startsWith("settings")) {
      setRoute("/settings");
    } else if (label.startsWith("inspector")) {
      setRoute("/inspector");
    } else if (label.startsWith("quick-preview")) {
      setRoute("/quick-preview");
    } else if (label.startsWith("cache-demo")) {
      setRoute("/cache-demo");
    }

    // Tell Tauri window is ready to be shown
    invoke("app_ready").catch(console.error);

    // Create Tauri-based client
    try {
      console.log("Creating SpacedriveClient with TauriTransport...");
      const transport = new TauriTransport(invoke, listen);
      const spacedrive = new SpacedriveClient(transport);
      setClient(spacedrive);
      console.log("Client created successfully");

      // Start event subscription
      spacedrive.subscribe().then(() => {
        console.log("Event subscription active");
      });
    } catch (err) {
      console.error("Failed to create client:", err);
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  console.log("App render state:", { hasClient: !!client, hasError: !!error, route });

  // Routes that don't need the client
  if (route === "/floating-controls") {
    return <FloatingControls />;
  }

  if (route === "/drag-overlay") {
    return <DragOverlay />;
  }

  if (route === "/drag-demo") {
    return <DragDemo />;
  }

  if (error) {
    console.log("Rendering error state");
    return (
      <div className="flex h-screen items-center justify-center bg-gray-950 text-white">
        <div className="text-center">
          <h1 className="text-2xl font-bold mb-4">Error</h1>
          <p className="text-red-400">{error}</p>
        </div>
      </div>
    );
  }

  if (!client) {
    console.log("Rendering loading state");
    return (
      <div className="flex h-screen items-center justify-center bg-gray-950 text-white">
        <div className="text-center">
          <div className="animate-pulse text-xl">Initializing client...</div>
          <p className="text-gray-400 text-sm mt-2">Check console for logs</p>
        </div>
      </div>
    );
  }

  console.log("Rendering Interface with client");

  // Route to different UIs based on window type
  if (route === "/settings") {
    return (
      <div className="flex h-screen items-center justify-center bg-gray-950 text-white">
        <div className="text-center">
          <h1 className="text-4xl font-bold mb-4">Settings</h1>
          <p className="text-gray-400">Settings UI will go here</p>
        </div>
      </div>
    );
  }

  if (route === "/inspector") {
    return (
      <div className="h-screen bg-app overflow-hidden">
        <Inspector showPopOutButton={false} />
      </div>
    );
  }

  if (route === "/cache-demo") {
    return <LocationCacheDemo />;
  }

  if (route === "/quick-preview") {
    return (
      <div className="h-screen bg-app overflow-hidden">
        <QuickPreview />
      </div>
    );
  }

  return (
    <PlatformProvider platform={platform}>
      <Explorer client={client} />
    </PlatformProvider>
  );
}

export default App;
