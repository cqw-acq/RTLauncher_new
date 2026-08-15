"use client";

import * as React from "react";
import { Fragment, jsx, jsxs } from "react/jsx-runtime";

import * as UI from "./public-ui";
import type { ThemeDefinition } from "./protocol";
import { THEME_API_VERSION } from "./protocol";

export interface ThemeHostBridge {
  readonly apiVersion: string;
  readonly React: typeof React;
  readonly jsxRuntime: Readonly<{
    Fragment: typeof Fragment;
    jsx: typeof jsx;
    jsxs: typeof jsxs;
  }>;
  readonly ui: typeof UI;
}

declare global {
  interface Window {
    __RTL_THEME_HOST__?: ThemeHostBridge;
    __RTL_THEME_REGISTER__?: (definition: ThemeDefinition) => void;
  }
}

let references = 0;
let previousBridge: ThemeHostBridge | undefined;

const HOST_BRIDGE: ThemeHostBridge = Object.freeze({
  apiVersion: THEME_API_VERSION,
  React,
  jsxRuntime: Object.freeze({ Fragment, jsx, jsxs }),
  ui: Object.freeze({ ...UI }),
});

export function acquireThemeHostBridge(): () => void {
  if (typeof window === "undefined") {
    throw new Error("The Theme host bridge requires a browser window.");
  }
  if (references === 0) {
    previousBridge = window.__RTL_THEME_HOST__;
    window.__RTL_THEME_HOST__ = HOST_BRIDGE;
  }
  references += 1;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    references -= 1;
    if (references > 0) return;
    if (previousBridge) window.__RTL_THEME_HOST__ = previousBridge;
    else delete window.__RTL_THEME_HOST__;
    previousBridge = undefined;
  };
}

export function getThemeHostBridge(): ThemeHostBridge {
  return HOST_BRIDGE;
}
