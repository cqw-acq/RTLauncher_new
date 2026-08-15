export type OpenP2PStatus =
  | "idle"
  | "not_installed"
  | "installed"
  | "starting"
  | "running"
  | "stopping"
  | "error";

export type RunMode = "host" | "join";

export type MultiplayerState = {
  status: OpenP2PStatus;
  errorMsg: string | null;
  runMode: RunMode | null;
  roomInfo: string | null;
  logText: string;
};

export const initialMultiplayerState: MultiplayerState = {
  status: "idle",
  errorMsg: null,
  runMode: null,
  roomInfo: null,
  logText: "",
};

export type MultiplayerAction =
  | { type: "statusResolved"; installed: boolean; running: boolean }
  | { type: "operationStarted"; operation: "start" | "stop" }
  | { type: "started"; mode: RunMode; roomInfo: string }
  | { type: "stopped" }
  | { type: "failed"; message: string }
  | { type: "logReceived"; text: string }
  | { type: "logCleared" };

export function multiplayerReducer(
  state: MultiplayerState,
  action: MultiplayerAction
): MultiplayerState {
  switch (action.type) {
    case "statusResolved":
      if (!action.installed) {
        return {
          ...state,
          status: "not_installed",
          errorMsg: null,
          runMode: null,
          roomInfo: null,
        };
      }
      if (action.running) {
        return { ...state, status: "running", errorMsg: null };
      }
      return {
        ...state,
        status: "installed",
        errorMsg: null,
        runMode: null,
        roomInfo: null,
      };
    case "operationStarted":
      return {
        ...state,
        status: action.operation === "start" ? "starting" : "stopping",
        errorMsg: null,
        ...(action.operation === "start" ? { logText: "" } : {}),
      };
    case "started":
      return {
        ...state,
        status: "running",
        errorMsg: null,
        runMode: action.mode,
        roomInfo: action.roomInfo,
      };
    case "stopped":
      return {
        ...state,
        status: "installed",
        errorMsg: null,
        runMode: null,
        roomInfo: null,
      };
    case "failed":
      return { ...state, status: "error", errorMsg: action.message };
    case "logReceived":
      if (!action.text) return state;
      return {
        ...state,
        logText:
          state.logText && !state.logText.endsWith("\n")
            ? `${state.logText}\n${action.text}`
            : state.logText + action.text,
      };
    case "logCleared":
      return { ...state, logText: "" };
  }
}
