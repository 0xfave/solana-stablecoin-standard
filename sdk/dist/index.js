"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.PrivacyClient = exports.ComplianceClient = exports.SolanaStablecoin = void 0;
exports.getInstructionDiscriminator = getInstructionDiscriminator;
exports.parseConfig = parseConfig;
const web3_js_1 = require("@solana/web3.js");
const spl_token_1 = require("@solana/spl-token");
const crypto_1 = require("crypto");
function getInstructionDiscriminator(name) {
    const hash = (0, crypto_1.createHash)("sha256").update(`global:${name}`).digest();
    return hash.slice(0, 8);
}
const PROGRAM_ID = "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw";
const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";
class SolanaStablecoin {
    constructor(connection, mint, config, authority, decimals = 9, minters = [], freezer = null, pauser = null, paused = false) {
        this._decimals = 9;
        this._minters = [];
        this._freezer = null;
        this._pauser = null;
        this._paused = false;
        this._connection = connection;
        this._mint = mint;
        this._config = config;
        this._authority = authority;
        this._programId = new web3_js_1.PublicKey(PROGRAM_ID);
        this._decimals = decimals;
        this._minters = minters;
        this._freezer = freezer;
        this._pauser = pauser;
        this._paused = paused;
    }
    get minters() {
        return this._minters;
    }
    get freezer() {
        return this._freezer;
    }
    get pauser() {
        return this._pauser;
    }
    get paused() {
        return this._paused;
    }
    static async create(connection, params) {
        const { authority, decimals, supplyCap } = params;
        const mintKeypair = web3_js_1.Keypair.generate();
        const [config] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("stablecoin"), mintKeypair.publicKey.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const tx = new web3_js_1.Transaction();
        const extensions = [spl_token_1.ExtensionType.PermanentDelegate];
        const mintSpace = (0, spl_token_1.getMintLen)(extensions);
        const lamports = await connection.getMinimumBalanceForRentExemption(mintSpace);
        tx.add(web3_js_1.SystemProgram.createAccount({
            fromPubkey: authority.publicKey,
            newAccountPubkey: mintKeypair.publicKey,
            lamports,
            space: mintSpace,
            programId: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
        }));
        // Always initialize permanent delegate extension
        const initPermanentDelegateIx = (0, spl_token_1.createInitializePermanentDelegateInstruction)(mintKeypair.publicKey, config, new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID));
        tx.add(initPermanentDelegateIx);
        const initMintIx = (0, spl_token_1.createInitializeMintInstruction)(mintKeypair.publicKey, decimals, authority.publicKey, authority.publicKey, new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID));
        tx.add(initMintIx);
        const initIx = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                { pubkey: config, isWritable: true, isSigner: false },
                { pubkey: mintKeypair.publicKey, isWritable: true, isSigner: false },
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
                supplyCap
                    ? (() => {
                        const buf = Buffer.alloc(9);
                        buf.writeUInt8(1, 0);
                        buf.writeBigUInt64LE(BigInt(supplyCap), 1);
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
        return new SolanaStablecoin(connection, mintKeypair.publicKey, config, authority.publicKey, decimals);
    }
    static async fetch(connection, mint) {
        const [config] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("stablecoin"), mint.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const configInfo = await connection.getAccountInfo(config);
        if (!configInfo?.data)
            return null;
        const data = configInfo.data;
        if (data.length < 8)
            return null;
        const expectedDiscriminator = (0, crypto_1.createHash)("sha256")
            .update("account:StablecoinConfig")
            .digest()
            .slice(0, 8);
        const actualDiscriminator = data.slice(0, 8);
        if (!actualDiscriminator.equals(expectedDiscriminator)) {
            return null;
        }
        try {
            const parsed = parseConfig(data);
            return new SolanaStablecoin(connection, mint, config, parsed.masterAuthority, parsed.decimals, parsed.minters, parsed.freezer, parsed.pauser, parsed.paused);
        }
        catch (e) {
            console.error("Failed to parse config:", e);
            return null;
        }
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
    get connection() {
        return this._connection;
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
            data: Buffer.concat([
                getInstructionDiscriminator("mint_tokens"),
                amountBuffer,
            ]),
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
        const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
        view.setBigUint64(0, BigInt(amount), true);
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
            data: Buffer.concat([
                getInstructionDiscriminator("burn_tokens"),
                amountBuffer,
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async transfer(params) {
        const { from, to, fromOwner, toOwner, amount, authority } = params;
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), this._config.toBuffer()], this._programId);
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), this._config.toBuffer()], this._programId);
        const [senderBlacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), this._config.toBuffer(), fromOwner.toBuffer()], this._programId);
        const [receiverBlacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), this._config.toBuffer(), toOwner.toBuffer()], this._programId);
        const [senderAllowlist] = await web3_js_1.PublicKey.findProgramAddress([
            Buffer.from("allowlist"),
            privacyModule.toBuffer(),
            fromOwner.toBuffer(),
        ], this._programId);
        const [receiverAllowlist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("allowlist"), privacyModule.toBuffer(), toOwner.toBuffer()], this._programId);
        const amountBuffer = Buffer.alloc(8);
        const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
        view.setBigUint64(0, BigInt(amount), true);
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: complianceModule, isWritable: false, isSigner: false },
                { pubkey: senderBlacklist, isWritable: false, isSigner: false },
                { pubkey: receiverBlacklist, isWritable: false, isSigner: false },
                { pubkey: privacyModule, isWritable: false, isSigner: false },
                { pubkey: senderAllowlist, isWritable: false, isSigner: false },
                { pubkey: receiverAllowlist, isWritable: false, isSigner: false },
                { pubkey: this._mint, isWritable: false, isSigner: false },
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
    async proposeMasterAuthority(newAuthority, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("propose_master_authority"),
                newAuthority.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async acceptMasterAuthority(newAuthority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: newAuthority.publicKey, isWritable: false, isSigner: true },
            ],
            data: getInstructionDiscriminator("accept_master_authority"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = newAuthority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await newAuthority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async updatePaused(paused, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_paused"),
                Buffer.from([paused ? 1 : 0]),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async updateFreezer(newFreezer, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_freezer"),
                newFreezer.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async updatePauser(newPauser, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_pauser"),
                newPauser.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async updateSupplyCap(newSupplyCap, authority) {
        const supplyCapBuffer = Buffer.alloc(9);
        if (newSupplyCap !== null) {
            supplyCapBuffer.writeUInt8(1, 0);
            supplyCapBuffer.writeBigUInt64LE(BigInt(newSupplyCap), 1);
        }
        else {
            supplyCapBuffer.writeUInt8(0, 0);
        }
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_supply_cap"),
                supplyCapBuffer,
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async thawAccount(account, authority) {
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: this._mint, isWritable: false, isSigner: false },
                { pubkey: account, isWritable: true, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
                {
                    pubkey: new web3_js_1.PublicKey(TOKEN_2022_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: getInstructionDiscriminator("thaw_account"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async attachComplianceModule(blacklister, authority, transferHookProgram, permanentDelegate) {
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), this._config.toBuffer()], this._programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: complianceModule, isWritable: true, isSigner: false },
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (payer)
                {
                    pubkey: new web3_js_1.PublicKey(SYSTEM_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("attach_compliance_module"),
                blacklister.toBuffer(),
                Buffer.from([transferHookProgram ? 1 : 0]),
                ...(transferHookProgram ? [transferHookProgram.toBuffer()] : []),
                Buffer.from([permanentDelegate ? 1 : 0]),
                ...(permanentDelegate ? [permanentDelegate.toBuffer()] : []),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async detachComplianceModule(authority) {
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), this._config.toBuffer()], this._programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: complianceModule, isWritable: true, isSigner: false },
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
            ],
            data: getInstructionDiscriminator("detach_compliance_module"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async attachPrivacyModule(allowlistAuthority, confidentialTransfersEnabled, authority) {
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), this._config.toBuffer()], this._programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: privacyModule, isWritable: true, isSigner: false },
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (payer)
                {
                    pubkey: new web3_js_1.PublicKey(SYSTEM_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("attach_privacy_module"),
                allowlistAuthority.toBuffer(),
                Buffer.from([confidentialTransfersEnabled ? 1 : 0]),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this._connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this._connection.sendRawTransaction(signed.serialize());
    }
    async detachPrivacyModule(authority) {
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), this._config.toBuffer()], this._programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId: this._programId,
            keys: [
                { pubkey: privacyModule, isWritable: true, isSigner: false },
                { pubkey: this._config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
            ],
            data: getInstructionDiscriminator("detach_privacy_module"),
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
    get privacy() {
        return new PrivacyClient(this);
    }
}
exports.SolanaStablecoin = SolanaStablecoin;
class ComplianceClient {
    constructor(stablecoin) {
        this.stablecoin = stablecoin;
    }
    get connection() {
        return this.stablecoin.connection;
    }
    get configAddress() {
        return this.stablecoin.configAddress;
    }
    get mintAddress() {
        return this.stablecoin.mintAddress;
    }
    // Helper: returns true if the compliance module PDA exists on-chain
    async isAttached() {
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), this.configAddress.toBuffer()], programId);
        const info = await this.connection.getAccountInfo(complianceModule);
        return info !== null && info.data.length > 0;
    }
    async blacklistAdd(address, reason, blacklister) {
        const config = this.stablecoin.configAddress;
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), config.toBuffer()], programId);
        const [blacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()], programId);
        const reasonBytes = Buffer.from(reason.slice(0, 128));
        const reasonLengthBuffer = Buffer.alloc(4);
        reasonLengthBuffer.writeUInt32LE(reasonBytes.length, 0);
        const ix = new web3_js_1.TransactionInstruction({
            programId,
            keys: [
                { pubkey: blacklist, isWritable: true, isSigner: false },
                { pubkey: complianceModule, isWritable: false, isSigner: false },
                { pubkey: config, isWritable: false, isSigner: false },
                { pubkey: blacklister.publicKey, isWritable: true, isSigner: true },
                { pubkey: address, isWritable: false, isSigner: false },
                {
                    pubkey: new web3_js_1.PublicKey(SYSTEM_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("blacklist_add"),
                reasonLengthBuffer,
                reasonBytes,
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = blacklister.publicKey;
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await blacklister.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
    }
    async blacklistRemove(address, authority) {
        const config = this.stablecoin.configAddress;
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), config.toBuffer()], programId);
        const [blacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()], programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId,
            keys: [
                { pubkey: blacklist, isWritable: true, isSigner: false },
                { pubkey: complianceModule, isWritable: false, isSigner: false },
                { pubkey: config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
                { pubkey: address, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
            ],
            data: getInstructionDiscriminator("blacklist_remove"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
    }
    async updateBlacklister(newBlacklister, authority) {
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), this.stablecoin.configAddress.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const ix = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                { pubkey: complianceModule, isWritable: true, isSigner: false },
                {
                    pubkey: this.stablecoin.configAddress,
                    isWritable: false,
                    isSigner: false,
                },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_blacklister"),
                newBlacklister.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
    }
    async updateTransferHook(newHookProgram, authority) {
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), this.stablecoin.configAddress.toBuffer()], new web3_js_1.PublicKey(PROGRAM_ID));
        const hookData = Buffer.alloc(33);
        if (newHookProgram) {
            hookData.writeUInt8(1, 0);
            hookData.set(newHookProgram.toBuffer(), 1);
        }
        else {
            hookData.writeUInt8(0, 0);
        }
        const ix = new web3_js_1.TransactionInstruction({
            programId: new web3_js_1.PublicKey(PROGRAM_ID),
            keys: [
                { pubkey: complianceModule, isWritable: true, isSigner: false },
                {
                    pubkey: this.stablecoin.configAddress,
                    isWritable: false,
                    isSigner: false,
                },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_transfer_hook"),
                hookData,
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
    }
    async seize(params) {
        const { from, to, sourceOwner, amount, seizer } = params;
        const config = this.stablecoin.configAddress;
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [complianceModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("compliance"), config.toBuffer()], programId);
        const [blacklist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("blacklist"), config.toBuffer(), sourceOwner.toBuffer()], programId);
        const amountBuffer = Buffer.alloc(8);
        const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
        view.setBigUint64(0, BigInt(amount), true);
        const ix = new web3_js_1.TransactionInstruction({
            programId,
            keys: [
                { pubkey: config, isWritable: false, isSigner: false },
                { pubkey: complianceModule, isWritable: false, isSigner: false },
                {
                    pubkey: this.stablecoin.mintAddress,
                    isWritable: false,
                    isSigner: false,
                },
                { pubkey: blacklist, isWritable: false, isSigner: false },
                { pubkey: from, isWritable: true, isSigner: false },
                { pubkey: to, isWritable: true, isSigner: false },
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
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await seizer.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
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
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
    }
    async thaw(account, authority) {
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
            data: getInstructionDiscriminator("thaw_account"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return this.connection.sendRawTransaction(signed.serialize());
    }
}
exports.ComplianceClient = ComplianceClient;
class PrivacyClient {
    constructor(stablecoin) {
        this.stablecoin = stablecoin;
    }
    // Helper: returns true if the privacy module PDA exists on-chain
    async isAttached() {
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), this.stablecoin.configAddress.toBuffer()], programId);
        const info = await this.stablecoin.connection.getAccountInfo(privacyModule);
        return info !== null && info.data.length > 0;
    }
    async allowlistAdd(address, authority) {
        const connection = this.stablecoin.connection;
        const config = this.stablecoin.configAddress;
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), config.toBuffer()], programId);
        const [allowlist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("allowlist"), privacyModule.toBuffer(), address.toBuffer()], programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId,
            keys: [
                { pubkey: allowlist, isWritable: true, isSigner: false },
                { pubkey: privacyModule, isWritable: false, isSigner: false },
                { pubkey: config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // allowlist_authority (payer)
                { pubkey: address, isWritable: false, isSigner: false },
                {
                    pubkey: new web3_js_1.PublicKey(SYSTEM_PROGRAM_ID),
                    isWritable: false,
                    isSigner: false,
                },
            ],
            data: getInstructionDiscriminator("allowlist_add"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return connection.sendRawTransaction(signed.serialize());
    }
    async allowlistRemove(address, authority) {
        const connection = this.stablecoin.connection;
        const config = this.stablecoin.configAddress;
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), config.toBuffer()], programId);
        const [allowlist] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("allowlist"), privacyModule.toBuffer(), address.toBuffer()], programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId,
            keys: [
                { pubkey: allowlist, isWritable: true, isSigner: false },
                { pubkey: privacyModule, isWritable: false, isSigner: false },
                { pubkey: config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // allowlist_authority
                { pubkey: address, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
            ],
            data: getInstructionDiscriminator("allowlist_remove"),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return connection.sendRawTransaction(signed.serialize());
    }
    async updateAllowlistAuthority(newAuthority, authority) {
        const connection = this.stablecoin.connection;
        const config = this.stablecoin.configAddress;
        const programId = new web3_js_1.PublicKey(PROGRAM_ID);
        const [privacyModule] = await web3_js_1.PublicKey.findProgramAddress([Buffer.from("privacy"), config.toBuffer()], programId);
        const ix = new web3_js_1.TransactionInstruction({
            programId,
            keys: [
                { pubkey: privacyModule, isWritable: true, isSigner: false },
                { pubkey: config, isWritable: false, isSigner: false },
                { pubkey: authority.publicKey, isWritable: false, isSigner: true },
            ],
            data: Buffer.concat([
                getInstructionDiscriminator("update_allowlist_authority"),
                newAuthority.toBuffer(),
            ]),
        });
        const tx = new web3_js_1.Transaction().add(ix);
        tx.feePayer = authority.publicKey;
        tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
        const signed = await authority.signTransaction(tx);
        return connection.sendRawTransaction(signed.serialize());
    }
}
exports.PrivacyClient = PrivacyClient;
function parseConfig(data) {
    let offset = 8; // skip discriminator
    const masterAuthority = new web3_js_1.PublicKey(data.slice(offset, offset + 32));
    offset += 32;
    const mint = new web3_js_1.PublicKey(data.slice(offset, offset + 32));
    offset += 32;
    const paused = data[offset] === 1;
    offset += 1;
    // supply_cap: Option<u64>
    const hasSupplyCap = data[offset] === 1;
    offset += 1;
    let supplyCap;
    if (hasSupplyCap) {
        supplyCap = data.readBigUInt64LE(offset);
        offset += 8;
    }
    // decimals: u8
    const decimals = data[offset];
    offset += 1;
    // bump: u8
    const bump = data[offset];
    offset += 1;
    // pending_master_authority: Option<Pubkey>
    const hasPending = data[offset] === 1;
    offset += 1;
    let pendingMasterAuthority;
    if (hasPending) {
        pendingMasterAuthority = new web3_js_1.PublicKey(data.slice(offset, offset + 32));
        offset += 32;
    }
    // minters: Vec<Pubkey>
    const mintersLen = data.readUInt32LE(offset);
    offset += 4;
    const minters = [];
    for (let m = 0; m < mintersLen && m < 10; m++) {
        minters.push(new web3_js_1.PublicKey(data.slice(offset, offset + 32)));
        offset += 32;
    }
    // freezer: Pubkey
    const freezer = new web3_js_1.PublicKey(data.slice(offset, offset + 32));
    offset += 32;
    // pauser: Pubkey
    const pauser = new web3_js_1.PublicKey(data.slice(offset, offset + 32));
    return {
        masterAuthority,
        mint,
        paused,
        supplyCap,
        decimals,
        bump,
        pendingMasterAuthority,
        minters,
        freezer,
        pauser,
    };
}
