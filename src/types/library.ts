export interface User {
  id: string;
  email: string;

  google_access_token: string;
  google_refresh_token: string;
}

export interface Library {
  id: string;
  name: string;
  object_count: number;
  total_size: number;
  encryption: Encryption;

  public: boolean;
  date_created: Date;
}

export interface UserLibrary {
  library_id: string;
  user_id: string;
  date_joined: Date;
  role: UserLibraryRole;
}

export enum Encryption {
  NONE,
  '128-AES',
  '192-AES',
  '256-AES'
}

export enum UserLibraryRole {
  OWNER,
  READ_ONLY
}
