import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";
import { suppressBrowserContextMenu } from "./lib/contextMenu";
import { applyAppTheme } from "./lib/theme";

applyAppTheme("system");
suppressBrowserContextMenu();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
