/* @refresh reload */
import { render, Suspense } from "solid-js/web";

import "./index.css";
import { App } from "./App";
import { queryClient, rspc, rspcClient } from "./rspc";

render(
  () => (
    <rspc.Provider client={rspcClient} queryClient={queryClient}>
      <Suspense fallback={null}>
        <App />
      </Suspense>
    </rspc.Provider>
  ),
  document.getElementById("root") as HTMLElement
);
