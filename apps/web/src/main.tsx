import { Shell } from "@sd/interface";
import { PlatformProvider } from "@sd/interface/platform";
import React from "react";
import ReactDOM from "react-dom/client";
import { platform } from "./platform";
import "@sd/interface/styles.css";

/**
 * Web entry point for Spacedrive server interface
 */
function App() {
  return (
    <PlatformProvider platform={platform}>
      <Shell />
    </PlatformProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
