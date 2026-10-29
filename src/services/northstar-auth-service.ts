import { invoke } from "@tauri-apps/api/core";
import type { ElyLoginResult, NorthstarAccount } from "../types/northstar-auth";

/** Rust owns account persistence and keyring access; tokens are never returned. */
export const NorthstarAuthService = {
  list: () => invoke<NorthstarAccount[]>("list_northstar_accounts"),
  createOffline: (username: string) =>
    invoke<NorthstarAccount>("create_offline_account", { username }),
  authenticateElyBy: (username: string, password: string, totp?: string) =>
    invoke<ElyLoginResult>("authenticate_ely_by", { username, password, totp: totp ?? null }),
  validateEly: (accountId: string) =>
    invoke<boolean>("validate_ely_account", { accountId }),
  refreshEly: (accountId: string) =>
    invoke<NorthstarAccount>("refresh_ely_account", { accountId }),
  setActive: (accountId: string) =>
    invoke<void>("set_northstar_active_account", { accountId }),
  remove: (accountId: string) =>
    invoke<void>("remove_northstar_account", { accountId }),
  setSkin: (accountId: string, path: string | null) =>
    invoke<void>("set_northstar_skin", { accountId, path }),
};
