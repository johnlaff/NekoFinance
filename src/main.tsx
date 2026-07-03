import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyMotionPreference } from "./lib/motion";

// Antes do primeiro render: o colapso de --dur-* precisa estar em <html> para as
// animações de entrada da primeira tela já nascerem certas.
applyMotionPreference();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
