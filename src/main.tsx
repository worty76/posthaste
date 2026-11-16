import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DetailsWindow } from "./details/DetailsWindow";
import { api, toAppError } from "./lib/ipc";
import { MainWindow } from "./main/MainWindow";
import type { AppInfo } from "./lib/types";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/ui.css";
import "./styles/details.css";
import "./styles/main.css";

function Fatal({ message }: { message: string }) {
  return (
    <div className="fatal" role="alert">
      <h1 className="fatal__title">Posthaste can't open its database</h1>
      <p className="fatal__text">{message}</p>
      <p className="fatal__text">
        Nothing has been deleted. Your notes file was left exactly as it was. Fix the problem above
        (for example free some disk space, or check the file's permissions), then start Posthaste
        again.
      </p>
    </div>
  );
}

// The windows still written in React load this bundle; the window's label
// decides which one it is. The capture and Today windows are Rust now: see ui/.
const label = getCurrentWindow().label;
const isDetails = label === "details";
if (isDetails) document.documentElement.classList.add("is-details");

const root = createRoot(document.getElementById("root")!);

function fallbackOs(): AppInfo["os"] {
  const ua = navigator.userAgent;
  return ua.includes("Mac") ? "macos" : ua.includes("Windows") ? "windows" : "linux";
}

api.appInfo().then(
  (info) => {
    root.render(
      <StrictMode>
        {isDetails ? (
          <DetailsWindow os={info.os} />
        ) : info.startup_error ? (
          <Fatal message={info.startup_error.message} />
        ) : (
          <MainWindow info={info} />
        )}
      </StrictMode>,
    );
  },
  (e) => {
    root.render(
      isDetails ? <DetailsWindow os={fallbackOs()} /> : <Fatal message={toAppError(e).message} />,
    );
  },
);
