"use client";

import { createSolanaClient } from "gill";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import React, { ReactNode, useState, useEffect, useCallback } from "react";
import { SssToken } from "@/lib/useSolana";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

interface SolanaWallet {
  isPhantom?: boolean;
  isConnected?: boolean;
  publicKey?: { toBytes: () => Uint8Array; toString: () => string };
  connect: () => Promise<{
    publicKey: { toBytes: () => Uint8Array; toString: () => string };
  }>;
  disconnect: () => Promise<void>;
  onAccountChange: (callback: (account: any) => void) => void;
  request: (params: { method: string; params?: any }) => Promise<any>;
  signMessage?: (
    message: Uint8Array,
    display?: string
  ) => Promise<{ signature: Uint8Array }>;
  signTransaction?: (transaction: any) => Promise<{ signature: Uint8Array }>;
}

interface SolanaWindow {
  solana?: SolanaWallet;
}

declare global {
  interface Window extends SolanaWindow {}
}

function getSolanaWallet() {
  if (typeof window === "undefined") return null;
  return window.solana || null;
}

function WalletProviderInner({ children }: { children: ReactNode }) {
  const [wallet, setWallet] = useState<SolanaWallet | null>(null);
  const [connected, setConnected] = useState(false);
  const [publicKey, setPublicKey] = useState<string | null>(null);
  const [selectedToken, setSelectedToken] = useState<SssToken | null>(null);

  useEffect(() => {
    const solWallet = getSolanaWallet();
    setWallet(solWallet || null);

    if (solWallet?.isConnected && solWallet.publicKey) {
      setConnected(true);
      setPublicKey(solWallet.publicKey.toString());
    }
  }, []);

  const connectWallet = useCallback(async () => {
    if (!wallet) return;
    try {
      const result = await wallet.connect();
      setConnected(true);
      setPublicKey(result.publicKey.toString());
    } catch (err) {
      console.error("Connection error:", err);
    }
  }, [wallet]);

  const disconnectWallet = useCallback(async () => {
    if (!wallet) return;
    try {
      await wallet.disconnect();
      setConnected(false);
      setPublicKey(null);
    } catch (err) {
      console.error("Disconnect error:", err);
    }
  }, [wallet]);

  return (
    <WalletContext.Provider
      value={{ connected, publicKey, connectWallet, disconnectWallet, wallet }}
    >
      <TokenContext.Provider value={{ selectedToken, setSelectedToken }}>
        {children}
      </TokenContext.Provider>
    </WalletContext.Provider>
  );
}

export default function SolanaProviders({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <WalletProviderInner>{children}</WalletProviderInner>
    </QueryClientProvider>
  );
}

interface WalletContextValue {
  connected: boolean;
  publicKey: string | null;
  connectWallet: () => Promise<void>;
  disconnectWallet: () => Promise<void>;
  wallet: SolanaWallet | null;
}

export const WalletContext = React.createContext<WalletContextValue>({
  connected: false,
  publicKey: null,
  connectWallet: async () => {},
  disconnectWallet: async () => {},
  wallet: null,
});

interface TokenContextValue {
  selectedToken: SssToken | null;
  setSelectedToken: (token: SssToken | null) => void;
}

export const TokenContext = React.createContext<TokenContextValue>({
  selectedToken: null,
  setSelectedToken: () => {},
});
