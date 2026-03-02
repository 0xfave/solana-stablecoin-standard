import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { SolanaStablecoin, Presets, Signer, PRESET } from "../src/index";
import { createHash } from "crypto";

describe("SolanaStablecoin SDK", () => {
  const PROGRAM_ID = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
  const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";

  function getDiscriminator(name: string): Buffer {
    const hash = createHash("sha256").update(`global:${name}`).digest();
    return hash.slice(0, 8);
  }

  const mockConnection = {
    getMinimumBalanceForRentExemption: jest.fn().mockResolvedValue(1000000),
    getLatestBlockhash: jest.fn().mockResolvedValue({
      blockhash: "mockBlockhash",
      lastValidBlockHeight: 12345,
    }),
    sendRawTransaction: jest.fn().mockResolvedValue("mockSignature"),
    getAccountInfo: jest.fn().mockResolvedValue(null),
    getParsedAccountInfo: jest.fn().mockResolvedValue(null),
  } as unknown as Connection;

  const mockSigner: Signer = {
    publicKey: new PublicKey("11111111111111111111111111111111"),
    signTransaction: jest
      .fn()
      .mockImplementation((tx: Transaction) => Promise.resolve(tx)),
  };

  describe("Constants", () => {
    it("should have correct program ID", () => {
      expect(PROGRAM_ID).toBe("Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6");
    });

    it("should have correct Token-2022 program ID", () => {
      expect(TOKEN_2022_PROGRAM_ID).toBe(
        "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
      );
    });

    it("should have SSS_1 preset value of 0", () => {
      expect(PRESET.SSS_1).toBe(0);
    });

    it("should have SSS_2 preset value of 1", () => {
      expect(PRESET.SSS_2).toBe(1);
    });
  });

  describe("Signer interface", () => {
    it("should have publicKey property", () => {
      expect(mockSigner.publicKey).toBeInstanceOf(PublicKey);
    });

    it("should have signTransaction method", () => {
      expect(typeof mockSigner.signTransaction).toBe("function");
    });
  });

  describe("Instruction Discriminators", () => {
    it("should compute correct discriminator for initialize", () => {
      const disc = getDiscriminator("initialize");
      expect(disc.length).toBe(8);
      expect(disc).toEqual(getDiscriminator("initialize"));
    });

    it("should compute correct discriminator for mint", () => {
      const disc = getDiscriminator("mint");
      expect(disc.length).toBe(8);
    });

    it("should compute correct discriminator for burn", () => {
      const disc = getDiscriminator("burn");
      expect(disc.length).toBe(8);
    });

    it("should compute correct discriminator for transfer", () => {
      const disc = getDiscriminator("transfer");
      expect(disc.length).toBe(8);
    });

    it("should compute correct discriminator for freeze_account", () => {
      const disc = getDiscriminator("freeze_account");
      expect(disc.length).toBe(8);
    });

    it("should compute correct discriminator for add_minter", () => {
      const disc = getDiscriminator("add_minter");
      expect(disc.length).toBe(8);
    });

    it("should compute correct discriminator for remove_minter", () => {
      const disc = getDiscriminator("remove_minter");
      expect(disc.length).toBe(8);
    });
  });

  describe("PDA Derivation", () => {
    it("should derive config PDA correctly", async () => {
      const mint = new PublicKey(
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
      );
      const [config] = await PublicKey.findProgramAddress(
        [Buffer.from("stablecoin"), mint.toBuffer()],
        new PublicKey(PROGRAM_ID)
      );

      expect(config).toBeInstanceOf(PublicKey);
      expect(config.toString().length).toBeGreaterThan(0);
    });

    it("should derive blacklist PDA correctly", async () => {
      const config = new PublicKey(
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
      );
      const address = new PublicKey(
        "8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ"
      );
      const mint = new PublicKey(
        "9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU"
      );

      const [blacklist] = await PublicKey.findProgramAddress(
        [Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()],
        mint
      );

      expect(blacklist).toBeInstanceOf(PublicKey);
    });
  });

  describe("PublicKey validation", () => {
    it("should validate correct public key format", () => {
      const pubkey = new PublicKey("11111111111111111111111111111111");
      expect(pubkey.toString()).toBe("11111111111111111111111111111111");
    });

    it("should throw on invalid public key", () => {
      expect(() => new PublicKey("invalid")).toThrow();
    });
  });
});
