"use client";

import { useContext, useState, useEffect, useCallback, useRef } from "react";
import { WalletContext, TokenContext } from "../app/providers";
import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  Keypair,
} from "@solana/web3.js";
import {
  createInitializeMintInstruction,
  TOKEN_2022_PROGRAM_ID,
  getMint,
} from "@solana/spl-token";
import {
  SolanaStablecoin,
  getInstructionDiscriminator,
} from "../../sdk/src/index";

const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL || "https://api.devnet.solana.com";

export interface SssToken {
  mint: string;
  config: string;
  authority: string;
  supply: string;
  decimals: number;
  name?: string;
  symbol?: string;
  paused: boolean;
  preset: number;
}

export function useSolana() {
  const { connected, publicKey, wallet, connectWallet, disconnectWallet } =
    useContext(WalletContext);
  const { selectedToken, setSelectedToken } = useContext(TokenContext);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tokens, setTokens] = useState<SssToken[]>([]);
  const fetchTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const walletAddress = publicKey || "";
  const connectionRef = useRef<Connection | null>(null);
  if (!connectionRef.current) {
    connectionRef.current = new Connection(RPC_URL, "confirmed");
  }
  const connection = connectionRef.current;

  const fetchTokens = useCallback(
    async (immediate = false) => {
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
          const programId = new PublicKey(
            "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6"
          );

          // Filter: master_authority at offset 8 matches wallet
          const filters = [
            { memcmp: { offset: 8, bytes: walletPubkey.toBase58() } },
          ];

          const accounts = await connection.getProgramAccounts(programId, {
            filters,
            commitment: "confirmed",
          });

          console.log(
            `Found ${accounts.length} config accounts with matching authority`
          );

          const sssTokens: SssToken[] = [];

          for (const { pubkey: configPubkey, account } of accounts) {
            try {
              const mintPubkey = new PublicKey(account.data.slice(40, 72));

              let preset = 0;
              try {
                const configData = await connection.getAccountInfo(
                  configPubkey
                );
                if (configData?.data) {
                  preset = configData.data[68] ?? 0;
                }
              } catch (e) {
                console.warn("Failed to get preset from config", e);
              }

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
                  preset,
                });
              } else {
                // Fallback: read mint directly
                const mintInfo = await getMint(
                  connection,
                  mintPubkey,
                  "confirmed",
                  TOKEN_2022_PROGRAM_ID
                );
                sssTokens.push({
                  mint: mintPubkey.toString(),
                  config: configPubkey.toString(),
                  authority: walletPubkey.toString(),
                  supply: mintInfo.supply.toString(),
                  decimals: mintInfo.decimals,
                  paused: false,
                  preset,
                });
              }
            } catch (e) {
              console.warn(
                `Failed to fetch details for config ${configPubkey.toString()}`,
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
    }
  }, [connected, publicKey, fetchTokens]);

  const getBalance = useCallback(
    async (addr: string) => {
      try {
        const pubkey = new PublicKey(addr);
        const bal = await connection.getBalance(pubkey);
        return bal / 1e9;
      } catch {
        return 0;
      }
    },
    [connection]
  );

  const createToken = useCallback(
    async (
      preset: number,
      decimals: number = 6,
      name: string = "Stablecoin",
      symbol: string = "STBL"
    ) => {
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

        const tx = new Transaction();
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
            authorityPubkey, // freeze authority = wallet initially
            TOKEN_2022_PROGRAM_ID
          )
        );

        // 3. Initialize stablecoin config
        const initIx = new TransactionInstruction({
          programId: new PublicKey(PROGRAM_ID),
          keys: [
            { pubkey: config, isWritable: true, isSigner: false },
            {
              pubkey: mintKeypair.publicKey,
              isWritable: true,
              isSigner: false,
            },
            { pubkey: authorityPubkey, isWritable: true, isSigner: true },
            {
              pubkey: TOKEN_2022_PROGRAM_ID,
              isWritable: false,
              isSigner: false,
            },
            {
              pubkey: SystemProgram.programId,
              isWritable: false,
              isSigner: false,
            },
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

        const signature = await connection.sendRawTransaction(
          signedTx.serialize(),
          {
            skipSimulation: true,
          }
        );

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
          preset,
        };
        setTokens((prev) => [...prev, newToken]);

        return stablecoin;
      } catch (err) {
        const errorMsg =
          err instanceof Error ? err.message : "Failed to create token";
        setError(errorMsg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [connected, wallet, publicKey, connection]
  );

  const addMinter = useCallback(
    async (token: SssToken, minterAddress: string) => {
      if (!connected || !wallet || !publicKey) {
        throw new Error("Wallet not connected");
      }

      setIsLoading(true);
      setError(null);

      try {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const minterPubkey = new PublicKey(minterAddress);

        const PROGRAM_ID = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
        const [configPubkey] = await PublicKey.findProgramAddress(
          [Buffer.from("stablecoin"), mintPubkey.toBuffer()],
          new PublicKey(PROGRAM_ID)
        );

        const stablecoin = new SolanaStablecoin(
          connection,
          mintPubkey,
          configPubkey,
          authorityPubkey,
          token.preset as 0 | 1
        );

        const signature = await stablecoin.addMinter(minterPubkey, {
          publicKey: authorityPubkey,
          signTransaction: async (tx) => {
            if (!wallet) throw new Error("Wallet not connected");
            return wallet.signTransaction(tx);
          },
        });

        await connection.confirmTransaction(signature, "confirmed");
        console.log("Minter added! Signature:", signature);

        return signature;
      } catch (err) {
        const errorMsg =
          err instanceof Error ? err.message : "Failed to add minter";
        setError(errorMsg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [connected, wallet, publicKey, connection]
  );

  const addFreezer = useCallback(
    async (token: SssToken, freezerAddress: string) => {
      if (!connected || !wallet || !publicKey) {
        throw new Error("Wallet not connected");
      }

      setIsLoading(true);
      setError(null);

      try {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);
        const freezerPubkey = new PublicKey(freezerAddress);

        const PROGRAM_ID = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
        const [configPubkey] = await PublicKey.findProgramAddress(
          [Buffer.from("stablecoin"), mintPubkey.toBuffer()],
          new PublicKey(PROGRAM_ID)
        );

        const discriminator = getInstructionDiscriminator("update_freezer");

        const ix = new TransactionInstruction({
          programId: new PublicKey(PROGRAM_ID),
          keys: [
            { pubkey: configPubkey, isWritable: true, isSigner: false },
            { pubkey: authorityPubkey, isWritable: false, isSigner: true },
          ],
          data: Buffer.concat([discriminator, freezerPubkey.toBuffer()]),
        });

        let tx = new Transaction().add(ix);
        tx.feePayer = authorityPubkey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

        const signed = await wallet.signTransaction(tx);
        const signature = await connection.sendRawTransaction(
          signed.serialize()
        );
        await connection.confirmTransaction(signature, "confirmed");
        console.log("Freezer added! Signature:", signature);

        return signature;
      } catch (err) {
        const errorMsg =
          err instanceof Error ? err.message : "Failed to add freezer";
        setError(errorMsg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [connected, wallet, publicKey, connection]
  );

  const sendTransaction = useCallback(
    async (transaction: unknown) => {
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
        const errorMsg =
          err instanceof Error ? err.message : "Transaction failed";
        setError(errorMsg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [connected, wallet]
  );

  const mint = useCallback(
    async (token: SssToken, recipientAddress: string, amount: number) => {
      if (!connected || !wallet || !publicKey) {
        throw new Error("Wallet not connected");
      }

      if (!recipientAddress || recipientAddress.trim() === "") {
        throw new Error("Recipient address is required");
      }

      let recipientPubkey: PublicKey;
      try {
        recipientPubkey = new PublicKey(recipientAddress);
      } catch {
        throw new Error("Invalid recipient address format");
      }

      if (!PublicKey.isOnCurve(recipientPubkey)) {
        throw new Error(
          "Recipient must be a valid wallet address (Ed25519 key)"
        );
      }

      setIsLoading(true);
      setError(null);

      try {
        const authorityPubkey = new PublicKey(publicKey);
        const mintPubkey = new PublicKey(token.mint);

        console.log("Authority:", authorityPubkey.toBase58());
        console.log("Recipient:", recipientPubkey.toBase58());
        console.log("Mint:", mintPubkey.toBase58());

        const configPubkey = new PublicKey(token.config);
        console.log("Config from token:", token.config);

        const stablecoin = new SolanaStablecoin(
          connection,
          mintPubkey,
          configPubkey,
          authorityPubkey,
          token.preset as 0 | 1
        );

        const {
          getAssociatedTokenAddress,
          getAccount,
          createAssociatedTokenAccountInstruction,
          TOKEN_2022_PROGRAM_ID,
        } = await import("@solana/spl-token");

        console.log("TOKEN_2022_PROGRAM_ID:", TOKEN_2022_PROGRAM_ID.toString());

        const ata = await getAssociatedTokenAddress(
          mintPubkey,
          recipientPubkey,
          false,
          TOKEN_2022_PROGRAM_ID
        );
        console.log("ATA:", ata.toBase58());

        let ataExists = false;
        try {
          await getAccount(connection, ata);
          ataExists = true;
          console.log("ATA exists");
        } catch {
          console.log("ATA does not exist, creating...");
        }

        if (!ataExists) {
          const createAtaIx = createAssociatedTokenAccountInstruction(
            authorityPubkey,
            ata,
            recipientPubkey,
            mintPubkey,
            TOKEN_2022_PROGRAM_ID
          );

          const tx = new Transaction().add(createAtaIx);
          tx.feePayer = authorityPubkey;
          tx.recentBlockhash = (
            await connection.getLatestBlockhash()
          ).blockhash;

          const signed = await wallet.signTransaction(tx);
          const sig = await connection.sendRawTransaction(signed.serialize());
          await connection.confirmTransaction(sig, "confirmed");
          console.log("ATA created!");
        }

        const amountInSmallest = Math.floor(
          amount * Math.pow(10, token.decimals)
        );

        const signature = await stablecoin.mint({
          recipient: ata,
          amount: amountInSmallest,
          minter: {
            publicKey: authorityPubkey,
            signTransaction: async (tx) => {
              if (!wallet) throw new Error("Wallet not connected");
              return wallet.signTransaction(tx);
            },
          },
        });

        await connection.confirmTransaction(signature, "confirmed");
        console.log("Tokens minted! Signature:", signature);

        return signature;
      } catch (err) {
        const errorMsg =
          err instanceof Error ? err.message : "Failed to mint tokens";
        setError(errorMsg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    [connected, wallet, publicKey, connection]
  );

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
              if (log.includes("TokensMinted")) {
                const parts = log.split(":");
                if (parts.length > 1) {
                  try {
                    const data = JSON.parse(parts[parts.length - 1].trim());
                    const amount =
                      (data.amount || 0) / Math.pow(10, token.decimals);
                    const to = data.to || "";
                    history.push({
                      amount: `+${amount.toLocaleString(undefined, {
                        minimumFractionDigits: 2,
                        maximumFractionDigits: 2,
                      })}`,
                      to: to ? `${to.slice(0, 4)}...${to.slice(-4)}` : "",
                      txn: `${sig.signature.slice(
                        0,
                        4
                      )}...${sig.signature.slice(-4)}`,
                      time: sig.blockTime ? getTimeAgo(sig.blockTime) : "",
                    });
                  } catch {
                    // Skip if can't parse
                  }
                }
              }
            }
          } catch {
            // Skip failed transactions
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

  const getTimeAgo = (timestamp: number): string => {
    const seconds = Math.floor(Date.now() / 1000 - timestamp);
    if (seconds < 60) return `${seconds}s ago`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
    if (seconds < 86400) return `${Math.floor(seconds / 3600)}h ago`;
    return `${Math.floor(seconds / 86400)}d ago`;
  };

  const addToken = useCallback((token: SssToken) => {
    setTokens((prev) => {
      if (prev.some((t) => t.mint === token.mint)) return prev;
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
    addMinter,
    addFreezer,
    mint,
    fetchMintHistory,
    addToken,
    isLoading,
    error,
    tokens,
    selectedToken,
    setSelectedToken,
    refreshTokens: fetchTokens,
  };
}
