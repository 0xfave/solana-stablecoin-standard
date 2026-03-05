"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Presets = exports.ComplianceClient = exports.SolanaStablecoin = exports.PRESET = void 0;
exports.getInstructionDiscriminator = getInstructionDiscriminator;
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const crypto_1 = require("crypto");
exports.PRESET = {
    SSS_1: 0,
    SSS_2: 1,
};
function getInstructionDiscriminator(name) {
    const hash = (0, crypto_1.createHash)("sha256").update(`global:${name}`).digest();
    return hash.slice(0, 8);
}
const PROGRAM_ID = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";
class SolanaStablecoin {
    constructor(connection, mint, config, authority, preset, decimals = 9) {
        this._decimals = 9;
        this._connection = connection;
        this._mint = mint;
        this._config = config;
        this._authority = authority;
        this._preset = preset;
        this._programId = new web3_js_1.PublicKey(PROGRAM_ID);
        this._decimals = decimals;
    }
    static async create(connection, params) {
        const { preset, authority, decimals, supplyCap } = params;
        const mintKeypair = web3_js_1.Keypair.generate();
        const [config] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("stablecoin"), mintKeypair.publicKey.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const tx = new web3_js_1.Transaction();
        // Calculate mint space - larger if preset=1 (SSS-2) needs Permanent Delegate extension
        const extensions = preset === exports.PRESET.SSS_2 ? [spl_token_1.ExtensionType.PermanentDelegate] : [];
        const mintSpace = (0, spl_token_1.getMintLen)(extensions);
        const lamports = await connection.getMinimumBalanceForRentExemption(mintSpace);
        tx.add(web3_js_1.SystemProgram.createAccount({
            fromPubkey: authority.publicKey,
            newAccountPubkey: mintKeypair.publicKey,
            lamports,
            space: mintSpace,
            programId: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
        }));
        // For preset=1 (SSS-2), add Permanent Delegate extension
        if (preset === exports.PRESET.SSS_2) {
            const initPermanentDelegateIx = (0, spl_token_1.createInitializePermanentDelegateInstruction)(mintKeypair.publicKey, config, new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID));
            tx.add(initPermanentDelegateIx);
        }
        const initMintIx = (0, spl_token_1.createInitializeMintInstruction)(mintKeypair.publicKey, decimals, authority.publicKey, // temporary mint authority (will be changed by program)
        authority.publicKey, // temporary freeze authority
        new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID));
        tx.add(initMintIx);
        const initIx = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                { pubkey: config, isWritable: true, isSigner: false },
                { pubkey: mintKeypair.publicKey, isWritable: true, isSigner: true },
                { pubkey: authority.publicKey, isWritable: true, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
                {
                    pubkey: new web3_js_1.PublicKey(SYSTEM_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("initialize"),
                Buffer.from([preset]),
                supplyCap
                    ? (() => {
                        const buf = Buffer.alloc(9);
                        buf.writeUInt8(1, 0);
                        buf.writeBigUInt64BE(BigInt(supplyCap), 1);
                        return buf;
                    })()
                    : Buffer.from([0]),
                Buffer.from([decimals]),
            ]),
        });
        tx.add(initIx);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signedTx = await authority.signTransaction(tx);
        signedTx.partialSign(mintKeypair);
        await connection.sendRawTransaction(signedTx.serialize());
        return new SolanaStablecoin(connection, mintKeypair.publicKey, config, authority.publicKey, preset);
    }
    static async fetch(connection, mint) {
        const [config] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("stablecoin"), mint.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const configInfo = await connection.getAccountInfo(config);
        if (!configInfo?.data)
            return null;
        const data = configInfo.data;
        if (data.length < 8)
            return null;
        const masterAuthority = new web3_js_1.PublicKey(data.slice(8, 40));
        const mintAddr = new web3_js_1.PublicKey(data.slice(40, 72));
        const preset = data[72];
        const paused = data[73] === 1;
        const mintInfo = await connection.getParsedAccountInfo(mint);
        let decimals = 9;
        if (mintInfo.value?.data) {
            const mintData = mintInfo.value.data;
            decimals = mintData.parsed?.info?.decimals ?? 9;
        }
        return new SolanaStablecoin(connection, mint, config, masterAuthority, preset, decimals);
    }
    get mintAddress() {
        return this._mint;
    }
    get configAddress() {
        return this._config;
    }
    get authorityAddress() {
        return this._authority;
    }
    get decimals() {
        return this._decimals;
    }
    get isCompliant() {
        return this._preset === exports.PRESET.SSS_2;
    }
    async getTotalSupply() {
        const mintInfo = await this._connection.getParsedAccountInfo(this._mint);
        if (!mintInfo.value?.data)
            return 0;
        const data = mintInfo.value.data;
        return parseFloat(data.parsed?.info?.supply?.uiAmountString ?? "0");
    }
    async mint(params) {
        const { recipient, amount, minter } = params;
        const amountBuffer = Buffer.alloc(8);
        const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
        view.setBigUint64(0, BigInt(amount), true);
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: this._mint, isWritable: true, isSigner: false },
                { pubkey: recipient, isWritable: true, isSigner: false },
                { pubkey: minter.publicKey, isWritable: false, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([getInstructionDiscriminator("mint"), amountBuffer]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = minter.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await minter.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async burn(params) {
        const { account, amount, authority } = params;
        const amountBuffer = Buffer.alloc(8);
        amountBuffer.writeBigUInt64BE(BigInt(amount));
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: this._mint, isWritable: true, isSigner: false },
                { pubkey: account, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([getInstructionDiscriminator("burn"), amountBuffer]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async transfer(params) {
        const { from, to, amount, authority } = params;
        const [senderBlacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), this._config.toBuffer(), from.toBuffer()], this._mint);
        const [receiverBlacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), this._config.toBuffer(), to.toBuffer()], this._mint);
        const amountBuffer = Buffer.alloc(8);
        amountBuffer.writeBigUInt64BE(BigInt(amount));
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: senderBlacklist, isWritable: false, isSigner: false },
                { pubkey: receiverBlacklist, isWritable: false, isSigner: false },
                { pubkey: from, isWritable: true, isSigner: false },
                { pubkey: to, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("transfer"),
                amountBuffer,
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    get compliance() {
        return new ComplianceClient(this);
    }
    async addMinter(newMinter, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("add_minter"),
                newMinter.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async removeMinter(minter, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("remove_minter"),
                minter.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
}
exports.SolanaStablecoin = SolanaStablecoin;
class ComplianceClient {
    constructor(stablecoin) {
        this.stablecoin = stablecoin;
    }
    async blacklistAdd(address, reason) {
        const config = this.stablecoin.configAddress;
        const [blacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const reasonBytes = Buffer.from(reason.slice(0, 200));
        const reasonLengthBuffer = Buffer.alloc(4);
        reasonLengthBuffer.writeUInt32LE(reasonBytes.length, 0);
        const discriminator = Buffer.from([
            0xfe, 0xb8, 0x83, 0xc8, 0x91, 0x32, 0x2b, 0xf4,
        ]);
        const ix = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                { pubkey: blacklist, isWritable: true, isSigner: false },
                { pubkey: config, isWritable: true, isSigner: false },
                {
                    pubkey: this.stablecoin.authorityAddress,
                    isWritable: true,
                    isSigner: true,
                },
                { pubkey: address, isWritable: false, isSigner: false },
                {
                    pubkey: new web3_js_1.PublicKey(SYSTEM_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([discriminator, reasonLengthBuffer, reasonBytes]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = this.stablecoin.authorityAddress;
        tx.recentBlockhash = (await new web3_js_1.Connection("").getLatestBlockhash()).blockhash;
        return "";
    }
    async seize(params) {
        const { from, to, amount, seizer } = params;
        const config = this.stablecoin.configAddress;
        const [blacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), config.toBuffer(), from.toBuffer()], this.stablecoin.mintAddress);
        const amountBuffer = Buffer.alloc(8);
        amountBuffer.writeBigUInt64BE(BigInt(amount));
        const ix = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                { pubkey: config, isWritable: false, isSigner: false },
                {
                    pubkey: this.stablecoin.mintAddress,
                    isWritable: false,
                    isSigner: false,
                },
                { pubkey: from, isWritable: true, isSigner: false },
                { pubkey: to, isWritable: true, isSigner: false },
                { pubkey: blacklist, isWritable: false, isSigner: false },
                { pubkey: seizer.publicKey, isWritable: false, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([getInstructionDiscriminator("seize"), amountBuffer]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = seizer.publicKey;
        tx.recentBlockhash = (await new web3_js_1.Connection("").getLatestBlockhash()).blockhash;
        const signed = await seizer.signTransaction(tx);
        return new web3_js_1.Connection("").sendRawTransaction(signed.serialize());
    }
    async freeze(account, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                {
                    pubkey: this.stablecoin.configAddress,
                    isWritable: false,
                    isSigner: false,
                },
                {
                    pubkey: this.stablecoin.mintAddress,
                    isWritable: false,
                    isSigner: false,
                },
                { pubkey: account, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: getInstructionDiscriminator("freeze_account"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await new web3_js_1.Connection("").getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return new web3_js_1.Connection("").sendRawTransaction(signed.serialize());
    }
}
exports.ComplianceClient = ComplianceClient;
exports.Presets = exports.PRESET;
