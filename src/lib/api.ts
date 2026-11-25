import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { GameStatus, WindowEntry, PackBackground,
  CharacterInfo,
  Config,
  CrosshairItem,
  CrosshairRender,
  FxEffect,
  Analysis,
  ImportOutcome,
  LastSession,
  OutfitMember,
  SquadRow,
  UpdateInfo,
  UpdateStatus,
  EssStatus,
  FocusInfo,
  ImportResult,
  RemovedPack,
  PackFormat,
  Manifest,
  MappingProposal,
  MonitorInfo,
  MoveLayerState,
  HotkeyFailure,
  ServiceIdSource,
  ClassId,
  AppInfo,
  PackMeta,
  PathsInfo,
  SessionSummary,
  Snapshot,
  SyncStatus,
  VoiceOverview,
  VoicePack,
} from "./types";

/** Is the page running in Tauri (otherwise: OBS browser source or browser preview)? */
export const inTauri = (): boolean => "__TAURI_INTERNALS__" in window;

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  saveConfig: (config: Config) => invoke<void>("save_config", { config }),
  lookupCharacter: (name: string) => invoke<CharacterInfo>("lookup_character", { name }),
  getStatus: () => invoke<EssStatus>("get_status"),
  getSnapshot: () => invoke<Snapshot | null>("get_snapshot"),
  listSessions: (limit = 100, offset = 0) =>
    invoke<SessionSummary[]>("list_sessions", { limit, offset }),
  getSession: (id: number) => invoke<Snapshot>("get_session", { id }),
  deleteSession: (id: number) => invoke<void>("delete_session", { id }),
  exportSession: (id: number, format: "json" | "csv" | "xlsx", path: string) =>
    invoke<void>("export_session", { id, format, path }),
  importSession: (path: string) => invoke<ImportOutcome>("import_session", { path }),
  sessionAnalysis: (ids: number[]) => invoke<Analysis[]>("session_analysis", { ids }),
  savePng: (path: string, dataUrl: string) => invoke<void>("save_png", { path, dataUrl }),
  squadRows: () => invoke<SquadRow[]>("squad_rows"),
  updateStatus: () => invoke<UpdateStatus>("update_status"),
  updateCheck: () => invoke<UpdateInfo | null>("update_check"),
  updateInstall: () => invoke<void>("update_install"),
  outfitMembers: (query: string) => invoke<{ outfit: string; members: OutfitMember[] }>("outfit_members", { query }),
  squadRemove: (characterId: string, deleteData: boolean) => invoke<number>("squad_remove", { characterId, deleteData }),
  startSession: (characterId: string) => invoke<void>("start_session", { characterId }),
  stopSession: (characterId: string) => invoke<void>("stop_session", { characterId }),
  resumeSession: (characterId: string) => invoke<void>("resume_session", { characterId }),
  lastSession: (characterId: string) => invoke<LastSession | null>("last_session", { characterId }),
  reopenSession: (characterId: string) => invoke<void>("reopen_session", { characterId }),
  overlayState: () =>
    invoke<{ edit: boolean; enabled: boolean; size_factor: number; selected: string }>("get_overlay_state"),
  overlayContentSize: (object: string, width: number, height: number) =>
    invoke<void>("overlay_content_size", { object, width, height }),
  overlaySetEdit: (edit: boolean) => invoke<void>("overlay_set_edit", { edit }),
  hotkeyStatus: () => invoke<HotkeyFailure[]>("hotkey_status"),
  serviceIdSource: () => invoke<ServiceIdSource>("service_id_source"),
  discordDefaultAvailable: () => invoke<boolean>("discord_default_available"),
  classIcons: () => invoke<Partial<Record<ClassId, string>>>("class_icons"),
  appInfo: () => invoke<AppInfo>("app_info"),
  moveLayerActivity: () => invoke<void>("move_layer_activity"),
  startupNotices: () => invoke<{ kind: string; detail: string }[]>("startup_notices"),
  moveLayerState: () => invoke<MoveLayerState | null>("move_layer_state"),
  moveLayerSet: (object: string, x: number, y: number) => invoke<void>("move_layer_set", { object, x, y }),
  moveLayerSelect: (object: string) => invoke<void>("move_layer_select", { object }),
  moveLayerEnd: (save: boolean) => invoke<void>("move_layer_end", { save }),
  moveLayerReset: (object: string) => invoke<void>("move_layer_reset", { object }),
  moveLayerMonitor: (object: string, monitor: string) => invoke<void>("move_layer_monitor", { object, monitor }),
  monitors: () => invoke<MonitorInfo[]>("list_monitors"),
  paths: () => invoke<PathsInfo>("get_paths"),

  voiceOverview: () => invoke<VoiceOverview>("voice_overview"),
  voiceGetPack: (id: string) => invoke<VoicePack>("voice_get_pack", { id }),
  voiceSaveManifest: (id: string, manifest: Manifest) =>
    invoke<void>("voice_save_manifest", { id, manifest }),
  voiceNewPack: (name: string, author: string, license: string) =>
    invoke<string>("voice_new_pack", { name, author, license }),
  voiceAddFile: (id: string, event: string, source: string) =>
    invoke<void>("voice_add_file", { id, event, source }),
  voiceImport: (path: string) => invoke<ImportResult>("voice_import", { path }),
  voicePackSize: (id: string) => invoke<{ files: number; bytes: number }>("voice_pack_size", { id }),
  voiceRemovePack: (id: string) => invoke<RemovedPack>("voice_remove_pack", { id }),
  voiceUndoRemove: (removed: RemovedPack) => invoke<void>("voice_undo_remove", { removed }),
  voiceSetOverride: (event: string, source: string) => invoke<string>("voice_set_override", { event, source }),
  voiceClearOverride: (event: string) => invoke<void>("voice_clear_override", { event }),
  voiceExport: (id: string, path: string, format: PackFormat) =>
    invoke<void>("voice_export", { id, path, format }),
  voicePreviewFile: (id: string, file: string) => invoke<string>("voice_preview_file", { id, file }),
  voicePreviewEvent: (event: string, count?: number) =>
    invoke<string>("voice_preview_event", { event, count }),
  voiceOutputDevices: () => invoke<string[]>("voice_output_devices"),
  voiceProposeMapping: (folder: string) =>
    invoke<MappingProposal[]>("voice_propose_mapping", { folder }),
  voiceCreateFromMapping: (folder: string, meta: PackMeta, mapping: MappingProposal[]) =>
    invoke<string>("voice_create_from_mapping", { request: { folder, meta, mapping } }),
  voicePreviewForeign: (folder: string, file: string) =>
    invoke<string>("voice_preview_foreign", { folder, file }),

  crosshairList: () => invoke<CrosshairItem[]>("crosshair_list"),
  crosshairImport: (path: string) => invoke<string>("crosshair_import", { path }),
  crosshairRender: () => invoke<CrosshairRender | null>("crosshair_render_info"),
  crosshairFxPreview: (kind: string, effect: FxEffect) => invoke<void>("crosshair_fx_preview", { kind, effect }),
  crosshairToggle: () => invoke<void>("crosshair_toggle"),
  focusProbe: () => invoke<FocusInfo>("focus_probe"),
  gameWindows: () => invoke<WindowEntry[]>("game_windows"),
  gameStatus: () => invoke<GameStatus>("game_status"),
  openFolder: (kind: string) => invoke<void>("open_folder", { kind }),
  toastBackground: () => invoke<PackBackground | null>("toast_background"),

  syncStatus: () => invoke<SyncStatus>("sync_status"),
  syncLogin: () => invoke<SyncStatus>("sync_login"),
  syncCancelLogin: () => invoke<void>("sync_cancel_login"),
  syncLogout: () => invoke<SyncStatus>("sync_logout"),
  syncDeleteAccount: () => invoke<SyncStatus>("sync_delete_account"),
  syncUploadSession: (id: number) => invoke<void>("sync_upload_session", { id }),
  syncUploadExisting: () => invoke<number>("sync_upload_existing"),
  syncOpenPrivacy: () => invoke<void>("sync_open_privacy"),
};

export function on<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => cb(e.payload));
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
