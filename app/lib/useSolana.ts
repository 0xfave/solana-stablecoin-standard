"use client";

import { useContext, useState, useEffect, useCallback, useRef } from "react";
import { WalletContext } from "../app/providers";
import { Connection, PublicKey, Transaction, TransactionInstruction, SystemProgram, Keypair } from "@solana/web3.js";
import { createInitializeMintInstruction, TOKEN_2022_PROGRAM_ID, getMint } from "@solana/spl-token";
import { SolanaStablecoin, getInstructionDiscriminator } from "../../sdk/src/index";

const RPC_URL = process.env.NEXT_PUBLIC_RPC_URL || "https://api.devnet.solana.com";

export interface SssToken {
  mint: string;
  config: string;
  authority: string;
  supply: string;
  decimals: number;
  name?: string;
  symbol?: string;
  paused: boolean;
}

export function useSolana() {
  const { connected, publicKey, wallet, connectWallet, disconnectWallet } = useContext(WalletContext);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tokens, setTokens] = useState<SssToken[]>([]);
  const fetchTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const walletAddress = publicKey || "";
  const connection = new Connection(RPC_URL, "confirmed");
  
  const fetchTokens = useCallback(async (immediate = false) => {
    if (fetchTimeoutRef.current) {
      clearTimeout(fetchTimeoutRef.current);
    }
  
    const executeFetch = async () => {
      if (!connected || !publicKey) {
        setTokens([]);
        return;
      }
  
      setIsLoading(true);
      try {
        const walletPubkey = new PublicKey(publicKey);
        const programId = new PublicKey("Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6");
  
        // Filter: master_authority at offset 8 matches wallet
        const filters = [
          { memcmp: { offset: 8, bytes: walletPubkey.toBase58() } }
        ];
  
        const accounts = await connection.getProgramAccounts(programId, {
          filters,
          commitment: "confirmed",
        });
  
        console.log(`Found ${accounts.length} config accounts with matching authority`);
  
        const sssTokens: SssToken[] = [];
  
        for (const { pubkey: configPubkey, account } of accounts) {
          try {
            // Extract mint at offset 40
            const mintPubkey = new PublicKey(account.data.slice(40, 72));
  
            // Attempt to fetch via SDK
            const sss = await SolanaStablecoin.fetch(connection, mintPubkey);
            if (sss) {
              const supply = await sss.getTotalSupply();
              sssTokens.push({
                mint: mintPubkey.toString(),
                config: configPubkey.toString(),
                authority: walletPubkey.toString(),
                supply: supply.toString(),
                decimals: sss.decimals,
                paused: false, // Update if SDK exposes paused
              });
            } else {
              // Fallback: read mint directly
              const mintInfo = await getMint(connection, mintPubkey, "confirmed", TOKEN_2022_PROGRAM_ID);
              sssTokens.push({
                mint: mintPubkey.toString(),
                config: configPubkey.toString(),
                authority: walletPubkey.toString(),
                supply: mintInfo.supply.toString(),
                decimals: mintInfo.decimals,
                paused: false,
              });
            }
          } catch (e) {
            console.warn(`Failed to fetch details for config ${configPubkey.toString()}`, e);
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
  }, [connected, publicKey, connection]);

  useEffect(() => {
    const timer = setTimeout(() => {
      fetchTokens();
    }, 30000);
    return () => clearTimeout(timer);
  }, [fetchTokens]);

  const getBalance = useCallback(async (addr: string) => {
    try {
      const pubkey = new PublicKey(addr);
      const bal = await connection.getBalance(pubkey);
      return bal / 1e9;
    } catch {
      return 0;
    }
  }, [connection]);

  const createToken = useCallback(async (preset: number, decimals: number = 6, name: string = "Stablecoin", symbol: string = "STBL") => {
    if (!connected || !wallet || !publicKey) {
      throw new Error("Wallet not connected");
    }

    setIsLoading(true);
    setError(null);

    try {
      const authorityPubkey = new PublicKey(publicKey);
      
      const mintKeypair = Keypair.generate();
      const [config] = await PublicKey.findProgramAddress(
        [Buffer.from("stablecoin"), mintKeypair.publicKey.toBuffer()],
        new PublicKey("Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6")
      );

      const PROGRAM_ID = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";

      let tx = new Transaction();
      const lamports = await connection.getMinimumBalanceForRentExemption(82);

      // 1. Create mint account
      tx.add(
        SystemProgram.createAccount({
          fromPubkey: authorityPubkey,
          newAccountPubkey: mintKeypair.publicKey,
          lamports,
          space: 82,
          programId: TOKEN_2022_PROGRAM_ID,
        })
      );

      // 2. Initialize mint with Token-2022 (mint authority = wallet, will be transferred to config by program)
      tx.add(
        createInitializeMintInstruction(
          mintKeypair.publicKey,
          decimals,
          authorityPubkey, // mint authority = wallet initially
          authorityPubkey,  // freeze authority = wallet initially
          TOKEN_2022_PROGRAM_ID
        )
      );

      // 3. Initialize stablecoin config
      const initIx = new TransactionInstruction({
        programId: new PublicKey(PROGRAM_ID),
        keys: [
          { pubkey: config, isWritable: true, isSigner: false },
          { pubkey: mintKeypair.publicKey, isWritable: true, isSigner: false },
          { pubkey: authorityPubkey, isWritable: true, isSigner: true },
          { pubkey: TOKEN_2022_PROGRAM_ID, isWritable: false, isSigner: false },
          { pubkey: SystemProgram.programId, isWritable: false, isSigner: false },
        ],
        data: Buffer.concat([
          getInstructionDiscriminator("initialize"),
          Buffer.from([preset]),
          Buffer.from([0]),
          Buffer.from([decimals]),
        ]),
      });
      tx.add(initIx);

      tx.feePayer = authorityPubkey;
      const { blockhash } = await connection.getLatestBlockhash();
      tx.recentBlockhash = blockhash;

      tx.partialSign(mintKeypair);

      const signedTx = await wallet.signTransaction(tx);

      const signature = await connection.sendRawTransaction(signedTx.serialize(), {
        skipSimulation: true,
      });
      
      await connection.confirmTransaction(signature, "confirmed");

      console.log("Token created! Signature:", signature);

      const stablecoin = new SolanaStablecoin(
        connection,
        mintKeypair.publicKey,
        config,
        authorityPubkey,
        preset as 0 | 1
      );

      // Add to tokens list immediately
      const newToken: SssToken = {
        mint: mintKeypair.publicKey.toString(),
        config: config.toString(),
        authority: authorityPubkey.toString(),
        supply: "0",
        decimals,
        name,
        symbol,
        paused: false,
      };
      setTokens(prev => [...prev, newToken]);

      return stablecoin;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Failed to create token";
      setError(errorMsg);
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, [connected, wallet, publicKey, connection]);

  const sendTransaction = useCallback(async (transaction: unknown) => {
    if (!connected || !wallet) {
      throw new Error("Wallet not connected");
    }

    setIsLoading(true);
    setError(null);

    try {
      if (wallet.signTransaction) {
        const signed = await wallet.signTransaction(transaction);
        return signed.signature ? Array.from(signed.signature) : [];
      }
      throw new Error("Wallet does not support transaction signing");
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : "Transaction failed";
      setError(errorMsg);
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, [connected, wallet]);

  const addToken = useCallback((token: SssToken) => {
    setTokens(prev => {
      if (prev.some(t => t.mint === token.mint)) return prev;
      return [...prev, token];
    });
  }, []);

  return {
    connected,
    walletAddress,
    publicKey,
    connectWallet,
    disconnectWallet,
    sendTransaction,
    getBalance,
    createToken,
    addToken,
    isLoading,
    error,
    tokens,
    refreshTokens: fetchTokens,
  };
}
