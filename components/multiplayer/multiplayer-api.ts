import { invoke } from "@tauri-apps/api/core";

export type MultiplayerStatusSnapshot = {
  installed: boolean;
  running: boolean;
};

export type MultiplayerPaths = {
  directory: string;
  executable: string;
};

export type MultiplayerApi = {
  readStatus: () => Promise<MultiplayerStatusSnapshot>;
  install: (sourcePath: string) => Promise<string>;
  encodeRoomInfo: (roomName: string, port: string) => Promise<string>;
  startHost: (roomName: string) => Promise<string>;
  startJoin: (encodedInfo: string, playerName: string) => Promise<string>;
  stop: () => Promise<void>;
  isRunning: () => Promise<boolean>;
  pollLog: () => Promise<string>;
  getPaths: () => Promise<MultiplayerPaths>;
};

export const tauriMultiplayerApi: MultiplayerApi = {
  async readStatus() {
    const installed = await invoke<boolean>("mp_check_openp2p");
    if (!installed) return { installed: false, running: false };
    const running = await invoke<boolean>("mp_is_openp2p_running");
    return { installed: true, running };
  },
  install(sourcePath) {
    return invoke<string>("mp_install_openp2p", { srcPath: sourcePath });
  },
  encodeRoomInfo(roomName, port) {
    return invoke<string>("mp_encode_room_info", {
      roomName,
      portCount: port,
    });
  },
  startHost(roomName) {
    return invoke<string>("mp_start_openp2p_host", { roomName });
  },
  startJoin(encodedInfo, playerName) {
    return invoke<string>("mp_start_openp2p_join", {
      encodedValue: encodedInfo,
      playerName,
    });
  },
  stop() {
    return invoke<void>("mp_stop_openp2p");
  },
  isRunning() {
    return invoke<boolean>("mp_is_openp2p_running");
  },
  pollLog() {
    return invoke<string>("mp_poll_log");
  },
  async getPaths() {
    const [directory, executable] = await Promise.all([
      invoke<string>("mp_get_openp2p_dir"),
      invoke<string>("mp_get_openp2p_path"),
    ]);
    return { directory, executable };
  },
};
