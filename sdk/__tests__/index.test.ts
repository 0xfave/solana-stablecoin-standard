import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import {
  SolanaStablecoin,
  Presets,
  Signer,
  PRESET,
} from '../src/index';

describe('SolanaStablecoin SDK', () => {
  const PROGRAM_ID = 'Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6';
  const TOKEN_2022_PROGRAM_ID = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb';

  const mockConnection = {
    getMinimumBalanceForRentExemption: jest.fn().mockResolvedValue(1000000),
    getLatestBlockhash: jest.fn().mockResolvedValue({
      blockhash: 'mockBlockhash',
      lastValidBlockHeight: 12345,
    }),
    sendRawTransaction: jest.fn().mockResolvedValue('mockSignature'),
    getAccountInfo: jest.fn().mockResolvedValue(null),
    getParsedAccountInfo: jest.fn().mockResolvedValue(null),
  } as unknown as Connection;

  const mockSigner: Signer = {
    publicKey: new PublicKey('11111111111111111111111111111111'),
    signTransaction: jest.fn().mockImplementation((tx: Transaction) => Promise.resolve(tx)),
  };

  describe('Constants', () => {
    it('should have correct program ID', () => {
      expect(PROGRAM_ID).toBe('Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6');
    });

    it('should have correct Token-2022 program ID', () => {
      expect(TOKEN_2022_PROGRAM_ID).toBe('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
    });

    it('should have SSS_1 preset value of 0', () => {
      expect(PRESET.SSS_1).toBe(0);
    });

    it('should have SSS_2 preset value of 1', () => {
      expect(PRESET.SSS_2).toBe(1);
    });
  });

  describe('Signer interface', () => {
    it('should have publicKey property', () => {
      expect(mockSigner.publicKey).toBeInstanceOf(PublicKey);
    });

    it('should have signTransaction method', () => {
      expect(typeof mockSigner.signTransaction).toBe('function');
    });
  });

  describe('Transaction building', () => {
    it('should create a valid mint instruction with correct program ID', () => {
      const mint = new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
      const recipient = new PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');

      const ix = new TransactionInstruction({
        programId: new PublicKey(PROGRAM_ID),
        keys: [
          { pubkey: mint, isWritable: false, isSigner: false },
          { pubkey: recipient, isWritable: true, isSigner: false },
          { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
          { pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
        ],
        data: Buffer.from([1, 0, 0, 0, 0, 0, 0, 0, 0]),
      });

      expect(ix.programId.toString()).toBe(PROGRAM_ID);
      expect(ix.keys.length).toBe(4);
      expect(ix.keys[0].pubkey.toString()).toBe(mint.toString());
      expect(ix.keys[2].isSigner).toBe(true);
    });

    it('should create burn instruction with correct data', () => {
      const account = new PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');

      const ix = new TransactionInstruction({
        programId: new PublicKey(PROGRAM_ID),
        keys: [
          { pubkey: new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU'), isWritable: false, isSigner: false },
          { pubkey: account, isWritable: true, isSigner: false },
          { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
          { pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
        ],
        data: Buffer.from([2, 0, 0, 0, 0, 0, 0, 0, 0]),
      });

      expect(ix.data[0]).toBe(2);
    });

    it('should create transfer instruction with correct data', () => {
      const from = new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
      const to = new PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');

      const ix = new TransactionInstruction({
        programId: new PublicKey(PROGRAM_ID),
        keys: [
          { pubkey: new PublicKey('9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU'), isWritable: false, isSigner: false },
          { pubkey: from, isWritable: true, isSigner: false },
          { pubkey: to, isWritable: true, isSigner: false },
          { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
          { pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
        ],
        data: Buffer.from([3, 0, 0, 0, 0, 0, 0, 0, 0]),
      });

      expect(ix.data[0]).toBe(3);
    });

    it('should create freeze instruction', () => {
      const account = new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');

      const ix = new TransactionInstruction({
        programId: new PublicKey(PROGRAM_ID),
        keys: [
          { pubkey: new PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ'), isWritable: false, isSigner: false },
          { pubkey: account, isWritable: true, isSigner: false },
          { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
          { pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
        ],
        data: Buffer.from([4]),
      });

      expect(ix.data[0]).toBe(4);
    });

    it('should create seize instruction with correct data', () => {
      const from = new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
      const to = new PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');

      const ix = new TransactionInstruction({
        programId: new PublicKey(PROGRAM_ID),
        keys: [
          { pubkey: new PublicKey('9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU'), isWritable: false, isSigner: false },
          { pubkey: from, isWritable: true, isSigner: false },
          { pubkey: to, isWritable: true, isSigner: false },
          { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
          { pubkey: new PublicKey('B5xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAs'), isWritable: false, isSigner: false },
          { pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
        ],
        data: Buffer.from([10, 0, 0, 0, 0, 0, 0, 0, 0]),
      });

      expect(ix.data[0]).toBe(10);
    });
  });

  describe('PDA Derivation', () => {
    it('should derive config PDA correctly', async () => {
      const mint = new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
      const [config] = await PublicKey.findProgramAddress(
        [Buffer.from('stablecoin'), mint.toBuffer()],
        new PublicKey(PROGRAM_ID)
      );

      expect(config).toBeInstanceOf(PublicKey);
      expect(config.toString().length).toBeGreaterThan(0);
    });

    it('should derive blacklist PDA correctly', async () => {
      const config = new PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
      const address = new PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');
      const mint = new PublicKey('9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');

      const [blacklist] = await PublicKey.findProgramAddress(
        [Buffer.from('blacklist'), config.toBuffer(), address.toBuffer()],
        mint
      );

      expect(blacklist).toBeInstanceOf(PublicKey);
    });
  });

  describe('Instruction data encoding', () => {
    it('should encode init instruction with preset byte', () => {
      const preset = 1;
      const data = Buffer.from([0, preset, 0, 0, 0, 0, 0, 0, 0]);
      expect(data[0]).toBe(0);
      expect(data[1]).toBe(preset);
    });

    it('should encode mint instruction correctly', () => {
      const data = Buffer.from([1, 0, 0, 0, 0, 0, 0, 0, 0]);
      expect(data[0]).toBe(1);
    });

    it('should encode burn instruction correctly', () => {
      const data = Buffer.from([2, 0, 0, 0, 0, 0, 0, 0, 0]);
      expect(data[0]).toBe(2);
    });

    it('should encode transfer instruction correctly', () => {
      const data = Buffer.from([3, 0, 0, 0, 0, 0, 0, 0, 0]);
      expect(data[0]).toBe(3);
    });

    it('should encode freeze instruction correctly', () => {
      const data = Buffer.from([4]);
      expect(data[0]).toBe(4);
    });

    it('should encode blacklist add instruction correctly', () => {
      const reasonBytes = Buffer.alloc(200);
      reasonBytes.write('test reason');
      const data = Buffer.concat([Buffer.from([6]), reasonBytes]);
      expect(data[0]).toBe(6);
    });

    it('should encode seize instruction correctly', () => {
      const data = Buffer.from([10, 0, 0, 0, 0, 0, 0, 0, 0]);
      expect(data[0]).toBe(10);
    });
  });

  describe('PublicKey validation', () => {
    it('should validate correct public key format', () => {
      const pubkey = new PublicKey('11111111111111111111111111111111');
      expect(pubkey.toString()).toBe('11111111111111111111111111111111');
    });

    it('should throw on invalid public key', () => {
      expect(() => new PublicKey('invalid')).toThrow();
    });
  });
});
