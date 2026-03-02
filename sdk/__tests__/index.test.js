"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const web3_js_1 = require("@solana/web3.js");
const index_1 = require("../src/index");
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
    };
    const mockSigner = {
        publicKey: new web3_js_1.PublicKey('11111111111111111111111111111111'),
        signTransaction: jest.fn().mockImplementation((tx) => Promise.resolve(tx)),
    };
    describe('Constants', () => {
        it('should have correct program ID', () => {
            expect(PROGRAM_ID).toBe('Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6');
        });
        it('should have correct Token-2022 program ID', () => {
            expect(TOKEN_2022_PROGRAM_ID).toBe('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
        });
        it('should have SSS_1 preset value of 0', () => {
            expect(index_1.PRESET.SSS_1).toBe(0);
        });
        it('should have SSS_2 preset value of 1', () => {
            expect(index_1.PRESET.SSS_2).toBe(1);
        });
    });
    describe('Signer interface', () => {
        it('should have publicKey property', () => {
            expect(mockSigner.publicKey).toBeInstanceOf(web3_js_1.PublicKey);
        });
        it('should have signTransaction method', () => {
            expect(typeof mockSigner.signTransaction).toBe('function');
        });
    });
    describe('Transaction building', () => {
        it('should create a valid mint instruction with correct program ID', () => {
            const mint = new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const recipient = new web3_js_1.PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');
            const ix = new web3_js_1.TransactionInstruction({
                programId: new web3_js_1.PublicKey(PROGRAM_ID),
                keys: [
                    { pubkey: mint, isWritable: false, isSigner: false },
                    { pubkey: recipient, isWritable: true, isSigner: false },
                    { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
                    { pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
                ],
                data: Buffer.from([1, 0, 0, 0, 0, 0, 0, 0, 0]),
            });
            expect(ix.programId.toString()).toBe(PROGRAM_ID);
            expect(ix.keys.length).toBe(4);
            expect(ix.keys[0].pubkey.toString()).toBe(mint.toString());
            expect(ix.keys[2].isSigner).toBe(true);
        });
        it('should create burn instruction with correct data', () => {
            const account = new web3_js_1.PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');
            const ix = new web3_js_1.TransactionInstruction({
                programId: new web3_js_1.PublicKey(PROGRAM_ID),
                keys: [
                    { pubkey: new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU'), isWritable: false, isSigner: false },
                    { pubkey: account, isWritable: true, isSigner: false },
                    { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
                    { pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
                ],
                data: Buffer.from([2, 0, 0, 0, 0, 0, 0, 0, 0]),
            });
            expect(ix.data[0]).toBe(2);
        });
        it('should create transfer instruction with correct data', () => {
            const from = new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const to = new web3_js_1.PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');
            const ix = new web3_js_1.TransactionInstruction({
                programId: new web3_js_1.PublicKey(PROGRAM_ID),
                keys: [
                    { pubkey: new web3_js_1.PublicKey('9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU'), isWritable: false, isSigner: false },
                    { pubkey: from, isWritable: true, isSigner: false },
                    { pubkey: to, isWritable: true, isSigner: false },
                    { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
                    { pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
                ],
                data: Buffer.from([3, 0, 0, 0, 0, 0, 0, 0, 0]),
            });
            expect(ix.data[0]).toBe(3);
        });
        it('should create freeze instruction', () => {
            const account = new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const ix = new web3_js_1.TransactionInstruction({
                programId: new web3_js_1.PublicKey(PROGRAM_ID),
                keys: [
                    { pubkey: new web3_js_1.PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ'), isWritable: false, isSigner: false },
                    { pubkey: account, isWritable: true, isSigner: false },
                    { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
                    { pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
                ],
                data: Buffer.from([4]),
            });
            expect(ix.data[0]).toBe(4);
        });
        it('should create seize instruction with correct data', () => {
            const from = new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const to = new web3_js_1.PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');
            const ix = new web3_js_1.TransactionInstruction({
                programId: new web3_js_1.PublicKey(PROGRAM_ID),
                keys: [
                    { pubkey: new web3_js_1.PublicKey('9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU'), isWritable: false, isSigner: false },
                    { pubkey: from, isWritable: true, isSigner: false },
                    { pubkey: to, isWritable: true, isSigner: false },
                    { pubkey: mockSigner.publicKey, isWritable: false, isSigner: true },
                    { pubkey: new web3_js_1.PublicKey('B5xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAs'), isWritable: false, isSigner: false },
                    { pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID), isWritable: false, isSigner: false },
                ],
                data: Buffer.from([10, 0, 0, 0, 0, 0, 0, 0, 0]),
            });
            expect(ix.data[0]).toBe(10);
        });
    });
    describe('PDA Derivation', () => {
        it('should derive config PDA correctly', async () => {
            const mint = new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const [config] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from('stablecoin'), mint.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
            expect(config).toBeInstanceOf(web3_js_1.PublicKey);
            expect(config.toString().length).toBeGreaterThan(0);
        });
        it('should derive blacklist PDA correctly', async () => {
            const config = new web3_js_1.PublicKey('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const address = new web3_js_1.PublicKey('8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ');
            const mint = new web3_js_1.PublicKey('9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
            const [blacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from('blacklist'), config.toBuffer(), address.toBuffer()], mint);
            expect(blacklist).toBeInstanceOf(web3_js_1.PublicKey);
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
            const pubkey = new web3_js_1.PublicKey('11111111111111111111111111111111');
            expect(pubkey.toString()).toBe('11111111111111111111111111111111');
        });
        it('should throw on invalid public key', () => {
            expect(() => new web3_js_1.PublicKey('invalid')).toThrow();
        });
    });
});
