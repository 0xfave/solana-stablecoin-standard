"use client";

import { useContext, useState, useEffect, useCallback, useRef } from "react";
import { WalletContext, TokenContext } from "../app/providers";
import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  getAccount,
  createAssociatedTokenAccountInstruction,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import {
  SolanaStablecoin,
  getInstructionDiscriminator,
} from "../../sdk/src/index";

// ─── Constants ───────────────────────────────────────────────────────────────

const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL || "https://api.devnet.solana.com";

// Updated to match declare_id! in lib.rs
const PROGRAM_ID = new PublicKey(
  "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw"
);

const SYSTEM_PROGRAM = new PublicKey("11111111111111111111111111111111");

// ─── Types ────────────────────────────────────────────────────────────────────
export interface SssToken {
  mint: string;
  config: string;
  authority: string;
  supply: string;
  decimals: number;
  name?: string;
  symbol?: string;
  paused: boolean;
  complianceAttached: boolean;
  privacyAttached: boolean;
  minters?: string[];
  freezer?: string;
  blacklister?: string;
}

// ─── PDA Derivation Helpers ───────────────────────────────────────────────────

function getConfigPda(mint: PublicKey): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddress(
    [Buffer.from("stablecoin"), mint.toBuffer()],
    PROGRAM_ID
  );
}

function getCompliancePda(config: PublicKey): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddress(
    [Buffer.from("compliance"), config.toBuffer()],
    PROGRAM_ID
  );
}

function getPrivacyPda(config: PublicKey): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddress(
    [Buffer.from("privacy"), config.toBuffer()],
    PROGRAM_ID
  );
}

function getBlacklistPda(
  config: PublicKey,
  target: PublicKey
): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddress(
    [Buffer.from("blacklist"), config.toBuffer(), target.toBuffer()],
    PROGRAM_ID
  );
}

// seeds: [allowlist, privacy_module, wallet]  — NOT [allowlist, config, wallet]
function getAllowlistPda(
  privacyModule: PublicKey,
  target: PublicKey
): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddress(
    [Buffer.from("allowlist"), privacyModule.toBuffer(), target.toBuffer()],
    PROGRAM_ID
  );
}

// ─── Misc Utilities ───────────────────────────────────────────────────────────

function getTimeAgo(timestamp: number): string {
  const seconds = Math.floor(Date.now() / 1000 - timestamp);
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
  return `${Math.floor(seconds / 86400)}d ago`;
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

export function useSolana() {
  const { connected, publicKey, wallet, connectWallet, disconnectWallet } =
    useContext(WalletContext);
  const { selectedToken, setSelectedToken } = useContext(TokenContext);

  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tokens, setTokens] = useState<SssToken[]>([]);

  const fetchTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const prevPublicKeyRef = useRef<string | null>(null);

  const connectionRef = useRef<Connection | null>(null);
  if (!connectionRef.current) {
    connectionRef.current = new Connection(RPC_URL, "confirmed");
  }
  const connection = connectionRef.current;

  const walletAddress = publicKey || "";

  // ─── Shared helpers ───────────────────────────────────────────────────────

  const withLoading = useCallback(
    async <T>(fn: () => Promise<T>): Promise<T> => {
      setIsLoading(true);
      setError(null);
      try {
        return await fn();
      } catch (err) {
        const msg = err instanceof Error ? err.message : "Unknown error";
        setError(msg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const signAndSend = useCallback(
    async (tx: Transaction): Promise<string> => {
      if (!wallet?.signTransaction)
        throw new Error("Wallet does not support signTransaction");
      const signed = (await wallet.signTransaction(tx)) as Transaction;
      return connection.sendRawTransaction(signed.serialize());
    },
    [wallet, connection]
  );
  const buildSigner = useCallback(() => {
    if (!wallet || !publicKey) throw new Error("Wallet not connected");
    return {
      publicKey: new PublicKey(publicKey),
      signTransaction: (tx: Transaction) => {
        if (!wallet.signTransaction)
          throw new Error("Wallet does not support signTransaction");
        return wallet.signTransaction(tx) as Promise<Transaction>;
      },
    };
  }, [wallet, publicKey]);

  // ─── Clear state on wallet change ────────────────────────────────────────

  useEffect(() => {
    if (
      publicKey &&
      prevPublicKeyRef.current &&
      prevPublicKeyRef.current !== publicKey
    ) {
      setTokens([]);
      setSelectedToken(null);
    }
    prevPublicKeyRef.current = publicKey;
  }, [publicKey, setSelectedToken]);

  // ─── fetchTokens ──────────────────────────────────────────────────────────

  const fetchTokens = useCallback(
    async (immediate = false) => {
      if (fetchTimeoutRef.current) clearTimeout(fetchTimeoutRef.current);

      const executeFetch = async () => {
        if (!connected || !publicKey) {
          setTokens([]);
          return;
        }

        setIsLoading(true);
        try {
          const walletPubkey = new PublicKey(publicKey);

          const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
            commitment: "confirmed",
          });

          const sssTokens: SssToken[] = [];

          for (const { pubkey: configPubkey, account } of accounts) {
            try {
              const mintPubkey = new PublicKey(account.data.slice(40, 72));
              const mintInfo = await connection.getParsedAccountInfo(
                mintPubkey
              );
              const rawSupply =
                (mintInfo.value?.data as any)?.parsed?.info?.supply ?? "0";

              const sss = await SolanaStablecoin.fetch(connection, mintPubkey);
              if (!sss) continue;

              const isMasterAuthority =
                sss.authorityAddress.equals(walletPubkey);
              const isMinter = sss.minters.some((m) => m.equals(walletPubkey));
              const isFreezer = sss.freezer?.equals(walletPubkey);

              if (!isMasterAuthority && !isMinter && !isFreezer) continue;

              const supply = await sss.getTotalSupply();
              sssTokens.push({
                mint: mintPubkey.toString(),
                config: configPubkey.toString(),
                authority: sss.authorityAddress.toString(),
                supply: rawSupply,
                decimals: sss.decimals,
                paused: sss.paused,
                complianceAttached: await sss.compliance.isAttached(),
                privacyAttached: await sss.privacy.isAttached(),
                minters: sss.minters.map((m) => m.toString()),
                freezer: sss.freezer?.toString() ?? "",
              });
            } catch (e) {
              console.warn(
                `Failed to load config ${configPubkey.toString()}`,
                e
              );
            }
          }

          setTokens(sssTokens);
        } catch (err) {
          console.error("Error fetching tokens:", err);
          setTokens([]);
        } finally {
          setIsLoading(false);
        }
      };

      if (immediate) {
        await executeFetch();
      } else {
        fetchTimeoutRef.current = setTimeout(executeFetch, 30000);
      }
    },
    [connected, publicKey, connection]
  );

  useEffect(() => {
    if (connected && publicKey) {
      fetchTokens(true);
    } else {
      setTokens([]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected, publicKey]);

  // ─── getBalance ───────────────────────────────────────────────────────────

  const getBalance = useCallback(
    async (addr: string) => {
      try {
        const bal = await connection.getBalance(new PublicKey(addr));
        return bal / 1e9;
      } catch {
        return 0;
      }
    },
    [connection]
  );

  // ─── createToken ──────────────────────────────────────────────────────────
  // No preset — token starts as SSS-1; call attachComplianceModule to upgrade.

  const createToken = useCallback(
    async (decimals = 6, name: string, symbol: string, supplyCap?: number) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.create(connection, {
          name,
          symbol,
          decimals,
          supplyCap,
          authority: signer,
        });

        const newToken: SssToken = {
          mint: stablecoin.mintAddress.toString(),
          config: stablecoin.configAddress.toString(),
          authority: publicKey,
          supply: "0",
          decimals,
          name,
          symbol,
          paused: false,
          complianceAttached: false,
          privacyAttached: false,
        };
        setTokens((prev) => [...prev, newToken]);
        return stablecoin;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── attachComplianceModule ───────────────────────────────────────────────
  // Upgrades SSS-1 → SSS-2. blacklisterAddress will control blacklist actions.

  const attachComplianceModule = useCallback(
    async (token: SssToken, blacklisterAddress: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.attachComplianceModule(
          new PublicKey(blacklisterAddress),
          signer
        );
        await connection.confirmTransaction(signature, "confirmed");

        setTokens((prev) =>
          prev.map((t) =>
            t.mint === token.mint ? { ...t, complianceAttached: true } : t
          )
        );
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── detachComplianceModule ───────────────────────────────────────────────

  const detachComplianceModule = useCallback(
    async (token: SssToken) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.detachComplianceModule(signer);
        await connection.confirmTransaction(signature, "confirmed");

        setTokens((prev) =>
          prev.map((t) =>
            t.mint === token.mint ? { ...t, complianceAttached: false } : t
          )
        );
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── attachPrivacyModule ──────────────────────────────────────────────────

  const attachPrivacyModule = useCallback(
    async (
      token: SssToken,
      allowlistAuthority: string,
      confidentialTransfersEnabled = false
    ) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.attachPrivacyModule(
          new PublicKey(allowlistAuthority),
          confidentialTransfersEnabled,
          signer
        );
        await connection.confirmTransaction(signature, "confirmed");

        setTokens((prev) =>
          prev.map((t) =>
            t.mint === token.mint ? { ...t, privacyAttached: true } : t
          )
        );
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── detachPrivacyModule ──────────────────────────────────────────────────

  const detachPrivacyModule = useCallback(
    async (token: SssToken) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.detachPrivacyModule(signer);
        await connection.confirmTransaction(signature, "confirmed");

        setTokens((prev) =>
          prev.map((t) =>
            t.mint === token.mint ? { ...t, privacyAttached: false } : t
          )
        );
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── allowlistAdd ─────────────────────────────────────────────────────────

  const allowlistAdd = useCallback(
    async (token: SssToken, address: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.privacy.allowlistAdd(
          new PublicKey(address),
          signer
        );
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── allowlistRemove ──────────────────────────────────────────────────────

  const allowlistRemove = useCallback(
    async (token: SssToken, address: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.privacy.allowlistRemove(
          new PublicKey(address),
          signer
        );
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── addMinter ────────────────────────────────────────────────────────────

  const addMinter = useCallback(
    async (token: SssToken, minterAddress: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const signer = buildSigner();
        const stablecoin = await SolanaStablecoin.fetch(
          connection,
          new PublicKey(token.mint)
        );
        if (!stablecoin) throw new Error("Token not found on chain");

        const signature = await stablecoin.addMinter(
          new PublicKey(minterAddress),
          signer
        );
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, buildSigner]
  );

  // ─── addFreezer ───────────────────────────────────────────────────────────

  const addFreezer = useCallback(
    async (token: SssToken, freezerAddress: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const [configPubkey] = await getConfigPda(mintPubkey);

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: configPubkey, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
          ],
          data: Buffer.concat([
            getInstructionDiscriminator("update_freezer"),
            new PublicKey(freezerAddress).toBuffer(),
          ]),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── addBlacklister ───────────────────────────────────────────────────────
  // Routes through compliance_module PDA, not directly to config.

  const addBlacklister = useCallback(
    async (token: SssToken, blacklisterAddress: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const [configPubkey] = await getConfigPda(mintPubkey);
        const [complianceModule] = await getCompliancePda(configPubkey);

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: complianceModule, isWritable: true, isSigner: false },
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
          ],
          data: Buffer.concat([
            getInstructionDiscriminator("update_blacklister"),
            new PublicKey(blacklisterAddress).toBuffer(),
          ]),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── blacklistAdd ─────────────────────────────────────────────────────────
  // compliance_module PDA at index 1 (required by on-chain constraint).

  const blacklistAdd = useCallback(
    async (token: SssToken, address: string, reason: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const targetPubkey = new PublicKey(address);
        const [configPubkey] = await getConfigPda(mintPubkey);
        const [complianceModule] = await getCompliancePda(configPubkey);
        const [blacklistEntry] = await getBlacklistPda(
          configPubkey,
          targetPubkey
        );

        const reasonBuffer = Buffer.from(reason);
        const reasonLengthBuffer = Buffer.alloc(4);
        reasonLengthBuffer.writeUInt32LE(reasonBuffer.length, 0);

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: blacklistEntry, isWritable: true, isSigner: false },
            { pubkey: complianceModule, isWritable: false, isSigner: false },
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: authorityPubkey, isWritable: true, isSigner: true },
            { pubkey: targetPubkey, isWritable: false, isSigner: false },
            { pubkey: SYSTEM_PROGRAM, isWritable: false, isSigner: false },
          ],
          data: Buffer.concat([
            getInstructionDiscriminator("blacklist_add"),
            reasonLengthBuffer,
            reasonBuffer,
          ]),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── blacklistRemove ──────────────────────────────────────────────────────
  // compliance_module at index 1; authority at index 5 writable+signer
  // (receives lamports from `close = authority` on the blacklist PDA).

  const blacklistRemove = useCallback(
    async (token: SssToken, address: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const targetPubkey = new PublicKey(address);
        const [configPubkey] = await getConfigPda(mintPubkey);
        const [complianceModule] = await getCompliancePda(configPubkey);
        const [blacklistEntry] = await getBlacklistPda(
          configPubkey,
          targetPubkey
        );

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: blacklistEntry, isWritable: true, isSigner: false },
            { pubkey: complianceModule, isWritable: false, isSigner: false },
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
            { pubkey: targetPubkey, isWritable: false, isSigner: false },
            { pubkey: authorityPubkey, isWritable: true, isSigner: true }, // rent recipient
          ],
          data: getInstructionDiscriminator("blacklist_remove"),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── freeze ───────────────────────────────────────────────────────────────

  const freeze = useCallback(
    async (token: SssToken, walletAddr: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const [configPubkey] = await getConfigPda(mintPubkey);
        const accountPubkey = await getAssociatedTokenAddress(
          mintPubkey,
          new PublicKey(walletAddr),
          false,
          TOKEN_2022_PROGRAM_ID
        );

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: mintPubkey, isWritable: false, isSigner: false },
            { pubkey: accountPubkey, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
            {
              pubkey: TOKEN_2022_PROGRAM_ID,
              isWritable: false,
              isSigner: false,
            },
          ],
          data: getInstructionDiscriminator("freeze_account"),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── thaw ─────────────────────────────────────────────────────────────────

  const thaw = useCallback(
    async (token: SssToken, walletAddr: string) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const [configPubkey] = await getConfigPda(mintPubkey);
        const accountPubkey = await getAssociatedTokenAddress(
          mintPubkey,
          new PublicKey(walletAddr),
          false,
          TOKEN_2022_PROGRAM_ID
        );

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: mintPubkey, isWritable: false, isSigner: false },
            { pubkey: accountPubkey, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
            {
              pubkey: TOKEN_2022_PROGRAM_ID,
              isWritable: false,
              isSigner: false,
            },
          ],
          data: getInstructionDiscriminator("thaw_account"),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── mint ─────────────────────────────────────────────────────────────────

  const mint = useCallback(
    async (token: SssToken, recipientAddress: string, amount: number) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");
      if (!recipientAddress?.trim())
        throw new Error("Recipient address is required");

      let recipientPubkey: PublicKey;
      try {
        recipientPubkey = new PublicKey(recipientAddress);
      } catch {
        throw new Error("Invalid recipient address format");
      }
      if (!PublicKey.isOnCurve(recipientPubkey))
        throw new Error(
          "Recipient must be a valid wallet address (Ed25519 key)"
        );

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const signer = buildSigner();

        const ata = await getAssociatedTokenAddress(
          mintPubkey,
          recipientPubkey,
          false,
          TOKEN_2022_PROGRAM_ID
        );

        let ataExists = false;
        try {
          await getAccount(connection, ata);
          ataExists = true;
        } catch {
          // ATA doesn't exist yet
        }

        if (!ataExists) {
          const createAtaIx = createAssociatedTokenAccountInstruction(
            authorityPubkey,
            ata,
            recipientPubkey,
            mintPubkey,
            TOKEN_2022_PROGRAM_ID
          );
          const ataTx = new Transaction().add(createAtaIx);
          ataTx.feePayer = authorityPubkey;
          ataTx.recentBlockhash = (
            await connection.getLatestBlockhash()
          ).blockhash;
          const ataSig = await signAndSend(ataTx);
          await connection.confirmTransaction(ataSig, "confirmed");
        }

        const stablecoin = await SolanaStablecoin.fetch(connection, mintPubkey);
        if (!stablecoin) throw new Error("Token not found on chain");

        const amountInSmallest = Math.floor(
          amount * Math.pow(10, token.decimals)
        );

        const signature = await stablecoin.mint({
          recipient: ata,
          amount: amountInSmallest,
          minter: signer,
        });

        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [
      connected,
      wallet,
      publicKey,
      connection,
      withLoading,
      buildSigner,
      signAndSend,
    ]
  );

  // ─── seize ────────────────────────────────────────────────────────────────
  // compliance_module inserted between config and mint.

  const seize = useCallback(
    async (
      token: SssToken,
      fromWallet: string,
      toWallet: string,
      amount: number
    ) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const configPubkey = new PublicKey(token.config);
        const fromWalletPubkey = new PublicKey(fromWallet);
        const toWalletPubkey = new PublicKey(toWallet);

        const [complianceModule] = await getCompliancePda(configPubkey);

        const fromAta = await getAssociatedTokenAddress(
          mintPubkey,
          fromWalletPubkey,
          false,
          TOKEN_2022_PROGRAM_ID
        );
        const toAta = await getAssociatedTokenAddress(
          mintPubkey,
          toWalletPubkey,
          false,
          TOKEN_2022_PROGRAM_ID
        );
        const [blacklistPDA] = await getBlacklistPda(
          configPubkey,
          fromWalletPubkey
        );

        const fromAtaInfo = await connection.getAccountInfo(fromAta);
        if (!fromAtaInfo)
          throw new Error("Source token account does not exist");
        const toAtaInfo = await connection.getAccountInfo(toAta);

        const amountBuffer = Buffer.alloc(8);
        new DataView(amountBuffer.buffer).setBigUint64(0, BigInt(amount), true);

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: complianceModule, isWritable: false, isSigner: false },
            { pubkey: mintPubkey, isWritable: false, isSigner: false },
            { pubkey: blacklistPDA, isWritable: false, isSigner: false },
            { pubkey: fromAta, isWritable: true, isSigner: false },
            { pubkey: toAta, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
            {
              pubkey: TOKEN_2022_PROGRAM_ID,
              isWritable: false,
              isSigner: false,
            },
          ],
          data: Buffer.concat([
            getInstructionDiscriminator("seize"),
            amountBuffer,
          ]),
        });

        const tx = new Transaction();
        if (!toAtaInfo) {
          tx.add(
            createAssociatedTokenAccountInstruction(
              authorityPubkey,
              toAta,
              toWalletPubkey,
              mintPubkey,
              TOKEN_2022_PROGRAM_ID
            )
          );
        }
        tx.add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── pauseToken ───────────────────────────────────────────────────────────

  const pauseToken = useCallback(
    async (token: SssToken, paused: boolean) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const [configPubkey] = await getConfigPda(mintPubkey);

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: configPubkey, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
          ],
          data: Buffer.concat([
            getInstructionDiscriminator("update_paused"),
            Buffer.from([paused ? 1 : 0]),
          ]),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");

        setTokens((prev) =>
          prev.map((t) => (t.mint === token.mint ? { ...t, paused } : t))
        );
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── burnTokens ───────────────────────────────────────────────────────────
  // Discriminator is "burn_tokens" (not "burn").

  const burnTokens = useCallback(
    async (token: SssToken, fromWallet: string, amount: number) => {
      if (!connected || !wallet || !publicKey)
        throw new Error("Wallet not connected");

      return withLoading(async () => {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const fromPubkey = new PublicKey(fromWallet);
        const [configPubkey] = await getConfigPda(mintPubkey);

        const fromAta = await getAssociatedTokenAddress(
          mintPubkey,
          fromPubkey,
          false,
          TOKEN_2022_PROGRAM_ID
        );

        const amountBuffer = Buffer.alloc(8);
        new DataView(amountBuffer.buffer).setBigUint64(0, BigInt(amount), true);

        const ix = new TransactionInstruction({
          programId: PROGRAM_ID,
          keys: [
            { pubkey: configPubkey, isWritable: false, isSigner: false },
            { pubkey: mintPubkey, isWritable: true, isSigner: false },
            { pubkey: fromAta, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
            {
              pubkey: TOKEN_2022_PROGRAM_ID,
              isWritable: false,
              isSigner: false,
            },
          ],
          data: Buffer.concat([
            getInstructionDiscriminator("burn_tokens"),
            amountBuffer,
          ]),
        });

        const tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signature = await signAndSend(tx);
        await connection.confirmTransaction(signature, "confirmed");
        return signature;
      });
    },
    [connected, wallet, publicKey, connection, withLoading, signAndSend]
  );

  // ─── addToken (local state helper) ───────────────────────────────────────

  const addToken = useCallback((token: SssToken) => {
    setTokens((prev) => {
      if (prev.some((t) => t.mint === token.mint)) return prev;
      return [...prev, token];
    });
  }, []);

  // ─── fetchMintHistory ─────────────────────────────────────────────────────

  const fetchMintHistory = useCallback(
    async (token: SssToken) => {
      if (!connected) return [];
      try {
        const mintPubkey = new PublicKey(token.mint);
        const signatures = await connection.getSignaturesForAddress(
          mintPubkey,
          { limit: 20 }
        );
        const history: {
          amount: string;
          to: string;
          txn: string;
          time: string;
        }[] = [];

        for (const sig of signatures) {
          try {
            const tx = await connection.getParsedTransaction(sig.signature, {
              maxSupportedTransactionVersion: 0,
            });
            if (!tx?.meta?.logMessages) continue;

            for (const log of tx.meta.logMessages) {
              if (!log.includes("Program data:")) continue;
              try {
                const dataBuffer = Buffer.from(
                  log.replace("Program data: ", ""),
                  "base64"
                );
                if (dataBuffer.length < 8) continue;
                if (
                  dataBuffer.slice(0, 8).toString("hex") !== "cfd480c2af364018"
                )
                  continue;

                const to = new PublicKey(dataBuffer.slice(40, 72)).toString();
                const amount = new DataView(
                  dataBuffer.buffer,
                  dataBuffer.byteOffset
                ).getBigUint64(72, true);

                history.push({
                  amount: `+${(
                    Number(amount) / Math.pow(10, token.decimals)
                  ).toLocaleString(undefined, {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2,
                  })}`,
                  to: `${to.slice(0, 4)}...${to.slice(-4)}`,
                  txn: `${sig.signature.slice(0, 4)}...${sig.signature.slice(
                    -4
                  )}`,
                  time: sig.blockTime ? getTimeAgo(sig.blockTime) : "",
                });
              } catch {
                /* skip */
              }
            }
          } catch {
            /* skip */
          }
        }
        return history;
      } catch (err) {
        console.error("Error fetching mint history:", err);
        return [];
      }
    },
    [connected, connection]
  );

  // ─── fetchFreezeHistory ───────────────────────────────────────────────────

  const fetchFreezeHistory = useCallback(
    async (token: SssToken) => {
      if (!connected) return [];
      try {
        const mintPubkey = new PublicKey(token.mint);
        const signatures = await connection.getSignaturesForAddress(
          mintPubkey,
          { limit: 20 }
        );
        const accountActions: Map<
          string,
          { action: "freeze" | "thaw"; time: string; txn: string }
        > = new Map();

        for (const sig of signatures) {
          try {
            const tx = await connection.getParsedTransaction(sig.signature, {
              maxSupportedTransactionVersion: 0,
            });
            if (!tx?.meta?.logMessages) continue;

            for (const log of tx.meta.logMessages) {
              if (!log.includes("Program data:")) continue;
              try {
                const dataBuffer = Buffer.from(
                  log.replace("Program data: ", ""),
                  "base64"
                );
                if (dataBuffer.length < 40) continue;

                const discriminator = dataBuffer.slice(0, 8).toString("hex");
                const account = new PublicKey(
                  dataBuffer.slice(8, 40)
                ).toString();
                const txnShort = `${sig.signature.slice(
                  0,
                  4
                )}...${sig.signature.slice(-4)}`;
                const time = sig.blockTime ? getTimeAgo(sig.blockTime) : "";

                if (discriminator === "ddd63b1df63277ce") {
                  accountActions.set(account, {
                    action: "freeze",
                    txn: txnShort,
                    time,
                  });
                } else if (discriminator === "313f496981be2877") {
                  accountActions.set(account, {
                    action: "thaw",
                    txn: txnShort,
                    time,
                  });
                }
              } catch {
                /* skip */
              }
            }
          } catch {
            /* skip */
          }
        }

        return [...accountActions.entries()]
          .filter(([, data]) => data.action === "freeze")
          .map(([account, data]) => ({
            account: `${account.slice(0, 4)}...${account.slice(-4)}`,
            accountFull: account,
            action: "freeze" as const,
            txn: data.txn,
            time: data.time,
          }));
      } catch (err) {
        console.error("Error fetching freeze history:", err);
        return [];
      }
    },
    [connected, connection]
  );

  // ─── fetchSeizeHistory ────────────────────────────────────────────────────

  const fetchSeizeHistory = useCallback(
    async (token: SssToken) => {
      if (!connected) return [];
      try {
        const mintPubkey = new PublicKey(token.mint);
        const configPubkey = new PublicKey(token.config);
        const signatures = await connection.getSignaturesForAddress(
          mintPubkey,
          { limit: 20 }
        );

        const results: {
          from: string;
          fromFull: string;
          to: string;
          amount: string;
          txn: string;
          time: string;
        }[] = [];

        for (const sig of signatures) {
          try {
            const tx = await connection.getParsedTransaction(sig.signature, {
              maxSupportedTransactionVersion: 0,
            });
            if (!tx) continue;

            for (const ix of tx.transaction.message.instructions) {
              if (ix.programId.toString() !== PROGRAM_ID.toString()) continue;
              if (!("accounts" in ix) || ix.accounts.length < 7) continue;

              // config is at index 0 — filter to this token only
              if (ix.accounts[0].toString() !== configPubkey.toString())
                continue;

              const fromAta = ix.accounts[4].toString();
              const toAta = ix.accounts[5].toString();

              // Get amount from inner transferChecked instruction
              let amount = "?";
              try {
                const inner = tx.meta?.innerInstructions?.find(
                  (ii) =>
                    ii.index === tx.transaction.message.instructions.indexOf(ix)
                );
                const transferIx = inner?.instructions.find(
                  (i) => "parsed" in i && i.parsed?.type === "transferChecked"
                );
                if (transferIx && "parsed" in transferIx) {
                  const raw =
                    transferIx.parsed.info.tokenAmount?.uiAmountString ??
                    transferIx.parsed.info.amount;
                  amount = Number(raw).toLocaleString(undefined, {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2,
                  });
                }
              } catch {
                /* use fallback */
              }

              results.push({
                from: `${fromAta.slice(0, 4)}...${fromAta.slice(-4)}`,
                fromFull: fromAta,
                to: `${toAta.slice(0, 4)}...${toAta.slice(-4)}`,
                amount,
                txn: `${sig.signature.slice(0, 4)}...${sig.signature.slice(
                  -4
                )}`,
                time: sig.blockTime ? getTimeAgo(sig.blockTime) : "",
              });
            }
          } catch {
            /* skip */
          }
        }

        return results;
      } catch (err) {
        console.error("Error fetching seize history:", err);
        return [];
      }
    },
    [connected, connection]
  );

  // ─── fetchBlacklistHistory ────────────────────────────────────────────────

  const fetchBlacklistHistory = useCallback(
    async (token: SssToken) => {
      if (!connected) return [];
      try {
        const configPubkey = new PublicKey(token.config);
        const signatures = await connection.getSignaturesForAddress(
          configPubkey,
          { limit: 20 }
        );
        const blacklistAddDiscriminator = "03c44e886fc5bc72";
        const accountActions: Map<
          string,
          {
            action: "add" | "remove";
            reason?: string;
            time: string;
            txn: string;
          }
        > = new Map();

        for (const sig of signatures) {
          try {
            const tx = await connection.getParsedTransaction(sig.signature, {
              maxSupportedTransactionVersion: 0,
            });
            if (!tx?.meta?.logMessages) continue;

            for (const log of tx.meta.logMessages) {
              if (!log.includes("Program data:")) continue;
              try {
                const dataBuffer = Buffer.from(
                  log.replace("Program data: ", ""),
                  "base64"
                );
                if (dataBuffer.length < 8) continue;
                if (
                  dataBuffer.slice(0, 8).toString("hex") !==
                  blacklistAddDiscriminator
                )
                  continue;

                let reason: string | undefined;
                if (dataBuffer.length > 12) {
                  const reasonLength = dataBuffer.readUInt32LE(8);
                  if (reasonLength > 0 && reasonLength < 200) {
                    reason = dataBuffer
                      .slice(12, 12 + reasonLength)
                      .toString("utf8");
                  }
                }

                const accountKeys = tx.transaction.message.accountKeys;
                const keys = Array.isArray(accountKeys)
                  ? accountKeys
                  : Object.values(accountKeys);
                if (keys.length > 5) {
                  const targetAcc = keys[5];
                  const address =
                    typeof targetAcc === "string"
                      ? targetAcc
                      : (targetAcc as { pubkey: string }).pubkey;
                  if (address) {
                    accountActions.set(address.toString(), {
                      action: "add",
                      reason,
                      txn: `${sig.signature.slice(
                        0,
                        4
                      )}...${sig.signature.slice(-4)}`,
                      time: sig.blockTime ? getTimeAgo(sig.blockTime) : "",
                    });
                  }
                }
              } catch {
                /* skip */
              }
            }
          } catch {
            /* skip */
          }
        }

        return [...accountActions.entries()]
          .filter(([, data]) => data.action === "add")
          .map(([address, data]) => ({
            address: `${address.slice(0, 4)}...${address.slice(-4)}`,
            addressFull: address,
            action: "add" as const,
            reason: data.reason,
            txn: data.txn,
            time: data.time,
          }));
      } catch (err) {
        console.error("Error fetching blacklist history:", err);
        return [];
      }
    },
    [connected, connection]
  );

  // ─── fetchBlacklistEntries ────────────────────────────────────────────────

  const fetchBlacklistEntries = useCallback(
    async (token: SssToken) => {
      if (!connected) return [];
      try {
        const configPubkey = new PublicKey(token.config);
        const signatures = await connection.getSignaturesForAddress(
          configPubkey,
          { limit: 50 }
        );

        const targets: Set<string> = new Set();

        for (const sig of signatures) {
          try {
            const tx = await connection.getParsedTransaction(sig.signature, {
              maxSupportedTransactionVersion: 0,
            });
            if (!tx) continue;

            for (const ix of tx.transaction.message.instructions) {
              if (ix.programId.toString() !== PROGRAM_ID.toString()) continue;
              if (!("accounts" in ix) || ix.accounts.length < 6) continue;

              // accounts: [blacklist_entry, compliance_module, config, blacklister, target, system_program]
              const ixConfig = ix.accounts[2].toString();
              if (ixConfig !== configPubkey.toString()) continue;

              // index 4 is the target wallet
              targets.add(ix.accounts[4].toString());
            }
          } catch {
            /* skip */
          }
        }

        const result: {
          address: string;
          addressFull: string;
          reason?: string;
          txn: string;
          time: string;
        }[] = [];

        for (const target of targets) {
          try {
            const [blacklistPDA] = await getBlacklistPda(
              configPubkey,
              new PublicKey(target)
            );
            const accountInfo = await connection.getAccountInfo(blacklistPDA);
            if (!accountInfo || accountInfo.data.length === 0) continue;

            // Layout: discriminator(8) + blacklister(32) + reason_len(4) + reason_data(n) + ...
            let reason: string | undefined;
            try {
              const reasonLen = accountInfo.data.readUInt32LE(40); // 8 + 32
              if (reasonLen > 0 && reasonLen <= 128) {
                reason = accountInfo.data
                  .slice(44, 44 + reasonLen)
                  .toString("utf8");
              }
            } catch {
              /* no reason */
            }

            result.push({
              address: `${target.slice(0, 4)}...${target.slice(-4)}`,
              addressFull: target,
              reason,
              txn: "",
              time: "",
            });
          } catch {
            /* PDA doesn't exist — was removed */
          }
        }

        return result;
      } catch (err) {
        console.error("Error fetching blacklist entries:", err);
        return [];
      }
    },
    [connected, connection]
  );

  // ─── fetchBurnHistory ─────────────────────────────────────────────────────

  const fetchBurnHistory = useCallback(
    async (token: SssToken) => {
      if (!connected) return [];
      try {
        const mintPubkey = new PublicKey(token.mint);
        const signatures = await connection.getSignaturesForAddress(
          mintPubkey,
          { limit: 20 }
        );
        const history: {
          amount: string;
          from: string;
          txn: string;
          time: string;
        }[] = [];

        for (const sig of signatures) {
          try {
            const tx = await connection.getParsedTransaction(sig.signature, {
              maxSupportedTransactionVersion: 0,
            });
            if (!tx?.meta?.logMessages) continue;

            for (const log of tx.meta.logMessages) {
              if (!log.includes("Program data:")) continue;
              const dataBuffer = Buffer.from(
                log.replace("Program data: ", ""),
                "base64"
              );
              if (dataBuffer.length < 8) continue;
              if (dataBuffer.slice(0, 8).toString("hex") !== "e6ff2271e235e309")
                continue;

              const from = new PublicKey(dataBuffer.slice(40, 72)).toString();
              const amount = new DataView(
                dataBuffer.buffer,
                dataBuffer.byteOffset
              ).getBigUint64(72, true);

              history.push({
                amount: `-${(
                  Number(amount) / Math.pow(10, token.decimals)
                ).toLocaleString(undefined, {
                  minimumFractionDigits: 2,
                  maximumFractionDigits: 2,
                })}`,
                from: `${from.slice(0, 4)}...${from.slice(-4)}`,
                txn: `${sig.signature.slice(0, 4)}...${sig.signature.slice(
                  -4
                )}`,
                time: sig.blockTime ? getTimeAgo(sig.blockTime) : "",
              });
            }
          } catch {
            /* skip */
          }
        }
        return history;
      } catch (err) {
        console.error("Error fetching burn history:", err);
        return [];
      }
    },
    [connected, connection]
  );

  // ─── Return ───────────────────────────────────────────────────────────────

  return {
    // Wallet
    connected,
    walletAddress,
    publicKey,
    connectWallet,
    disconnectWallet,
    getBalance,
    // Token lifecycle
    createToken,
    // Module management
    attachComplianceModule,
    detachComplianceModule,
    attachPrivacyModule,
    detachPrivacyModule,
    allowlistAdd,
    allowlistRemove,
    // Role management
    addMinter,
    addFreezer,
    addBlacklister,
    // Compliance actions
    blacklistAdd,
    blacklistRemove,
    seize,
    // Token actions
    freeze,
    thaw,
    mint,
    burnTokens,
    pauseToken,
    // State helpers
    addToken,
    // History fetchers
    fetchMintHistory,
    fetchFreezeHistory,
    fetchSeizeHistory,
    fetchBlacklistHistory,
    fetchBlacklistEntries,
    fetchBurnHistory,
    // State
    isLoading,
    error,
    tokens,
    selectedToken,
    setSelectedToken,
    refreshTokens: fetchTokens,
  };
}
