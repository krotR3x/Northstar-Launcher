export type NorthstarAuthProvider = "microsoft" | "ely_by" | "offline";

export interface NorthstarAccountProfile {
  id: string;
  name: string;
}

export interface NorthstarAccount {
  account_id: string;
  profile: NorthstarAccountProfile;
  provider: NorthstarAuthProvider;
  active: boolean;
  skin_path: string | null;
}

export interface ElyLoginResult {
  account: NorthstarAccount;
  availableProfiles: NorthstarAccountProfile[];
}
