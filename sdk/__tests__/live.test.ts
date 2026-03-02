require("dotenv").config({
  path: require("path").resolve(__dirname, "..", ".env"),
});
import bs58 from "bs58";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { SolanaStablecoin, Signer, PRESET } from "../src/index";

const PROGRAM_ID =
  process.env.PROGRAM_ID || "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
const TOKEN_2022_PROGRAM_ID =
  process.env.TOKEN_2022_PROGRAM_ID ||
  "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const RPC_URL = process.env.RPC_URL || "https://api.devnet.solana.com";

let connection: Connection;

describe("SolanaStablecoin SDK - Live Tests on Devnet", () => {
  let wallet: Keypair;
  let stablecoin: SolanaStablecoin | null;

  beforeAll(async () => {
    if (!process.env.PRIVATE_KEY) {
      throw new Error("PRIVATE_KEY not set in .env");
    }

    let privateKeyArray: number[];
    const pk = process.env.PRIVATE_KEY.replace(/\s/g, "");
    console.log("PRIVATE_KEY length:", pk.length);
    console.log("PRIVATE_KEY prefix:", pk.substring(0, 20));

    if (pk.startsWith("[")) {
      privateKeyArray = JSON.parse(pk);
    } else if (/^[1-9A-HJ-NP-Za-km-z]+$/.test(pk)) {
      const decoded = bs58.decode(pk);
      privateKeyArray = Array.from(decoded);
    } else {
      throw new Error(`Invalid PRIVATE_KEY format. Length: ${pk.length}`);
    }

    wallet = Keypair.fromSecretKey(new Uint8Array(privateKeyArray));
    console.log("Wallet public key:", wallet.publicKey.toString());

    connection = new Connection(RPC_URL, "confirmed");

    const balance = await connection.getBalance(wallet.publicKey);
    console.log(`Wallet balance: ${balance / 1e9} SOL`);

    if (balance < 0.1 * 1e9) {
      console.log("Warning: Low balance, airdropping...");
      const airdropSig = await connection.requestAirdrop(wallet.publicKey, 1e9);
      await connection.confirmTransaction(airdropSig);
    }
  });

  describe("Fetch existing or create new stablecoin", () => {
    it("should fetch existing stablecoin", async () => {
      const mintAddress = process.env.MINT_ADDRESS;

      if (!mintAddress) {
        console.log(
          "MINT_ADDRESS not set in .env - live tests require existing stablecoin"
        );
        console.log("To test, set MINT_ADDRESS in .env");
        console.log(
          "Or create a stablecoin using CLI and provide the addresses"
        );
        return;
      }

      console.log("Using existing stablecoin from .env");
      stablecoin = await SolanaStablecoin.fetch(
        connection,
        new PublicKey(mintAddress)
      );

      if (stablecoin) {
        console.log(`Fetched stablecoin! Mint: ${stablecoin.mintAddress}`);
        console.log(`Config: ${stablecoin.configAddress}`);
        console.log(`Authority: ${stablecoin.authorityAddress}`);
      }

      expect(stablecoin).toBeDefined();
    });
  });

  describe("Mint tokens", () => {
    it("should mint tokens to a recipient", async () => {
      if (!stablecoin) {
        console.log("Skipping mint test - no stablecoin created");
        return;
      }

      console.log("Testing mint...");

      const recipient = Keypair.generate();
      const recipientATA = await getOrCreateATA(
        connection,
        recipient.publicKey,
        stablecoin.mintAddress
      );

      const minterSigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      try {
        const mintTx = await stablecoin.mint({
          recipient: recipientATA,
          amount: 1000,
          minter: minterSigner,
        });

        console.log(`Mint transaction: ${mintTx}`);
        expect(mintTx).toBeDefined();
      } catch (error) {
        console.error("Mint failed:", error);
        throw error;
      }
    });
  });

  describe("Add/Remove Minters", () => {
    it("should add a new minter", async () => {
      if (!stablecoin) {
        console.log("Skipping add minter test - no stablecoin created");
        return;
      }

      console.log("Testing add_minter...");

      const newMinter = Keypair.generate();

      const authoritySigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      try {
        const tx = await stablecoin.addMinter(
          newMinter.publicKey,
          authoritySigner
        );
        console.log(`Add minter transaction: ${tx}`);
        expect(tx).toBeDefined();
      } catch (error) {
        console.error("Add minter failed:", error);
        throw error;
      }
    });

    it("should remove a minter", async () => {
      if (!stablecoin) {
        console.log("Skipping remove minter test - no stablecoin created");
        return;
      }

      console.log("Testing remove_minter...");

      const newMinter = Keypair.generate();

      const authoritySigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      await stablecoin.addMinter(newMinter.publicKey, authoritySigner);

      try {
        const tx = await stablecoin.removeMinter(
          newMinter.publicKey,
          authoritySigner
        );
        console.log(`Remove minter transaction: ${tx}`);
        expect(tx).toBeDefined();
      } catch (error) {
        console.error("Remove minter failed:", error);
        throw error;
      }
    });
  });

  describe("Burn tokens", () => {
    it("should burn tokens", async () => {
      if (!stablecoin) {
        console.log("Skipping burn test - no stablecoin created");
        return;
      }

      console.log("Testing burn...");

      const userATA = await getOrCreateATA(
        connection,
        wallet.publicKey,
        stablecoin.mintAddress
      );

      const burnerSigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      try {
        const tx = await stablecoin.burn({
          account: userATA,
          amount: 100,
          authority: burnerSigner,
        });

        console.log(`Burn transaction: ${tx}`);
        expect(tx).toBeDefined();
      } catch (error) {
        console.error("Burn failed:", error);
        throw error;
      }
    });
  });

  describe("Transfer tokens", () => {
    it("should transfer tokens", async () => {
      if (!stablecoin) {
        console.log("Skipping transfer test - no stablecoin created");
        return;
      }

      console.log("Testing transfer...");

      const fromATA = await getOrCreateATA(
        connection,
        wallet.publicKey,
        stablecoin.mintAddress
      );
      const toUser = Keypair.generate();
      const toATA = await getOrCreateATA(
        connection,
        toUser.publicKey,
        stablecoin.mintAddress
      );

      const authoritySigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      try {
        const tx = await stablecoin.transfer({
          from: fromATA,
          to: toATA,
          amount: 100,
          authority: authoritySigner,
        });

        console.log(`Transfer transaction: ${tx}`);
        expect(tx).toBeDefined();
      } catch (error) {
        console.error("Transfer failed:", error);
        throw error;
      }
    });
  });

  describe("SSS-2 Compliance: Blacklist", () => {
    it("should add address to blacklist (SSS-2 only)", async () => {
      if (!stablecoin) {
        console.log("Skipping blacklist test - no stablecoin");
        return;
      }

      if (!stablecoin.isCompliant) {
        console.log("Skipping blacklist test - not SSS-2 preset");
        return;
      }

      console.log("Testing blacklistAdd...");

      const targetAddress = Keypair.generate().publicKey;
      const authoritySigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      try {
        const tx = await stablecoin.compliance.blacklistAdd(
          targetAddress,
          "Test reason"
        );
        console.log(`Blacklist add transaction: ${tx}`);
        expect(tx).toBeDefined();
      } catch (error) {
        console.error("Blacklist add failed:", error);
        throw error;
      }
    });

    it("should seize tokens from blacklisted account (SSS-2 only)", async () => {
      if (!stablecoin) {
        console.log("Skipping seize test - no stablecoin");
        return;
      }

      if (!stablecoin.isCompliant) {
        console.log("Skipping seize test - not SSS-2 preset");
        return;
      }

      console.log("Testing seize...");

      const victim = Keypair.generate();
      const victimATA = await getOrCreateATA(
        connection,
        victim.publicKey,
        stablecoin.mintAddress
      );

      const seizerSigner: Signer = {
        publicKey: wallet.publicKey,
        signTransaction: async (tx) => {
          tx.partialSign(wallet);
          return tx;
        },
      };

      try {
        const tx = await stablecoin.compliance.seize({
          from: victimATA,
          to: victimATA,
          amount: 10,
          seizer: seizerSigner,
        });
        console.log(`Seize transaction: ${tx}`);
        expect(tx).toBeDefined();
      } catch (error) {
        console.error("Seize failed:", error);
        throw error;
      }
    });
  });
});

async function getOrCreateATA(
  conn: Connection,
  owner: PublicKey,
  mint: PublicKey
): Promise<PublicKey> {
  const {
    getAssociatedTokenAddressSync,
    createAssociatedTokenAccountInstruction,
  } = await import("@solana/spl-token");

  const ata = getAssociatedTokenAddressSync(
    mint,
    owner,
    false,
    new PublicKey(TOKEN_2022_PROGRAM_ID)
  );

  const accountInfo = await conn.getAccountInfo(ata);
  if (!accountInfo) {
    console.log(`Creating ATA for ${owner.toString()}`);
  }

  return ata;
}
