const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:6000";

export interface MintRequest {
  id: string;
  user_wallet: string;
  amount: number;
  fiat_tx_id: string;
  custodian: string;
  status: string;
  signature?: string;
  confirmed_at?: string;
  error?: string;
}

export interface BurnRequest {
  id: string;
  user_wallet: string;
  token_account: string;
  amount: number;
  fiat_destination: string;
  custodian: string;
  status: string;
  signature?: string;
  confirmed_at?: string;
  error?: string;
}

export interface BlacklistEntry {
  address: string;
  reason: string;
  blacklister: string;
  timestamp: string;
  status: string;
}

export interface OnChainEvent {
  event_type: string;
  signature: string;
  slot: number;
  timestamp: string;
  data: Record<string, unknown>;
}

export interface CreateTokenParams {
  name: string;
  symbol: string;
  decimals: number;
  preset: number; // 0 = SSS-1, 1 = SSS-2
  supply_cap?: number;
}

export interface TokenCreated {
  mint: string;
  config: string;
  signature: string;
}

async function fetchApi<T>(endpoint: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options?.headers,
    },
  });
  
  if (!res.ok) {
    throw new Error(`API Error: ${res.status}`);
  }
  
  return res.json();
}

export const api = {
  // Token creation (via backend)
  createToken: (data: CreateTokenParams) =>
    fetchApi<TokenCreated>("/api/token/create", { method: "POST", body: JSON.stringify(data) }),

  // Mint endpoints
  createMint: (data: { user_wallet: string; amount: number; fiat_tx_id: string }) =>
    fetchApi<MintRequest>("/api/mint", { method: "POST", body: JSON.stringify(data) }),
  
  getMint: (id: string) =>
    fetchApi<MintRequest>(`/api/mint/${id}`),
  
  getMintsByWallet: (wallet: string) =>
    fetchApi<MintRequest[]>(`/api/mint/wallet/${wallet}`),

  // Burn endpoints
  createBurn: (data: { user_wallet: string; token_account: string; amount: number; fiat_destination: string }) =>
    fetchApi<BurnRequest>("/api/burn", { method: "POST", body: JSON.stringify(data) }),
  
  getBurn: (id: string) =>
    fetchApi<BurnRequest>(`/api/burn/${id}`),
  
  getBurnsByWallet: (wallet: string) =>
    fetchApi<BurnRequest[]>(`/api/burn/wallet/${wallet}`),

  // Blacklist endpoints
  checkBlacklist: (address: string) =>
    fetchApi<{ blacklisted: boolean }>(`/api/blacklist/check/${address}`),
  
  getBlacklist: () =>
    fetchApi<BlacklistEntry[]>("/api/blacklist"),

  // Events
  getEvents: (limit = 50) =>
    fetchApi<OnChainEvent[]>(`/api/events?limit=${limit}`),
  
  getEventsBySignature: (signature: string) =>
    fetchApi<OnChainEvent[]>(`/api/events/${signature}`),

  // Health
  getHealth: () =>
    fetchApi<{ status: string; rpc_url: string }>("/health"),
};
