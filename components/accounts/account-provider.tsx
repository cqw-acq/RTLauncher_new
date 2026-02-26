"use client";

import React, { createContext, useContext, useState } from "react";
import accountsData from "@/config/accounts.json";
import type { Account } from "@/types";

type AccountContextType = {
  profiles: Account[];
  selectedProfile: Account | null;
  selectProfile: (acc: Account) => void;
  reloadProfiles: () => void;
};

const AccountContext = createContext<AccountContextType | undefined>(undefined);

export function AccountProvider({ children }: { children: React.ReactNode }) {
  const [profiles, setProfiles] = useState<Account[]>(
    accountsData as Account[]
  );
  const [selectedProfile, setSelectedProfile] = useState<Account | null>(
    profiles[0] ?? null
  );

  const selectProfile = (acc: Account) => {
    setSelectedProfile(acc);
  };

  const reloadProfiles = () => {
    setProfiles(accountsData as Account[]);
  };

  return (
    <AccountContext.Provider
      value={{ profiles, selectedProfile, selectProfile, reloadProfiles }}
    >
      {children}
    </AccountContext.Provider>
  );
}

export function useAccountContext() {
  const ctx = useContext(AccountContext);
  if (!ctx)
    throw new Error("useAccountContext must be used within AccountProvider");
  return ctx;
}
