"use client";

import { useState } from "react";
import { useSolana, SssToken } from "@/lib/useSolana";

export default function TokenTable() {
  const {
    connected,
    tokens,
    isLoading,
    refreshTokens,
    createToken,
    attachComplianceModule,
    detachComplianceModule,
    attachPrivacyModule,
    detachPrivacyModule,
    pauseToken,
    selectedToken,
    setSelectedToken,
  } = useSolana();

  // ─── Create token ─────────────────────────────────────────────────────────
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showSuccessModal, setShowSuccessModal] = useState(false);
  const [createdToken, setCreatedToken] = useState<{
    name: string;
    symbol: string;
    mint: string;
  } | null>(null);
  const [creating, setCreating] = useState(false);
  const [tokenName, setTokenName] = useState("");
  const [tokenSymbol, setTokenSymbol] = useState("");

  // ─── Compliance module ────────────────────────────────────────────────────
  const [showComplianceModal, setShowComplianceModal] = useState(false);
  const [complianceToken, setComplianceToken] = useState<SssToken | null>(null);
  const [blacklisterAddress, setBlacklisterAddress] = useState("");
  const [complianceLoading, setComplianceLoading] = useState(false);

  // ─── Privacy module ───────────────────────────────────────────────────────
  const [showPrivacyModal, setShowPrivacyModal] = useState(false);
  const [privacyToken, setPrivacyToken] = useState<SssToken | null>(null);
  const [allowlistAuthority, setAllowlistAuthority] = useState("");
  const [privacyLoading, setPrivacyLoading] = useState(false);

  // ─── Pause ────────────────────────────────────────────────────────────────
  const [pausingToken, setPausingToken] = useState<string | null>(null);

  // ─── Supply formatter ─────────────────────────────────────────────────────
  const formatSupply = (supply: string, decimals: number) => {
    try {
      const num = Number(supply) / Math.pow(10, decimals);
      if (num >= 1_000_000_000) return `${(num / 1_000_000_000).toFixed(3)}B`;
      if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(3)}M`;
      if (num >= 1_000) return `${(num / 1_000).toFixed(3)}K`;
      return num.toLocaleString(undefined, { maximumFractionDigits: 3 });
    } catch {
      return supply;
    }
  };

  // ─── Tier badge ───────────────────────────────────────────────────────────
  const getTier = (token: SssToken) => {
    if (token.privacyAttached)
      return { label: "SSS-3", color: "bg-purple-500/20 text-purple-400" };
    if (token.complianceAttached)
      return { label: "SSS-2", color: "bg-blue-500/20 text-blue-400" };
    return { label: "SSS-1", color: "bg-slate-500/20 text-slate-400" };
  };

  // ─── Handlers — create ────────────────────────────────────────────────────
  const handleCreateToken = async () => {
    if (!tokenName || !tokenSymbol) {
      alert("Please fill in all fields");
      return;
    }
    setCreating(true);
    try {
      const result = await createToken(6, tokenName, tokenSymbol);
      setShowCreateModal(false);
      setCreatedToken({
        name: tokenName,
        symbol: tokenSymbol,
        mint: result.mintAddress.toString(),
      });
      setShowSuccessModal(true);
      setTokenName("");
      setTokenSymbol("");
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to create token");
    } finally {
      setCreating(false);
    }
  };

  // ─── Handlers — compliance ────────────────────────────────────────────────
  const openComplianceModal = (token: SssToken) => {
    setComplianceToken(token);
    setBlacklisterAddress("");
    setShowComplianceModal(true);
  };

  const handleAttachCompliance = async () => {
    if (!complianceToken || !blacklisterAddress) return;
    setComplianceLoading(true);
    try {
      await attachComplianceModule(complianceToken, blacklisterAddress);
      setShowComplianceModal(false);
      setBlacklisterAddress("");
    } catch (err) {
      alert(
        err instanceof Error
          ? err.message
          : "Failed to attach compliance module"
      );
    } finally {
      setComplianceLoading(false);
    }
  };

  const handleDetachCompliance = async () => {
    if (!complianceToken) return;
    setComplianceLoading(true);
    try {
      await detachComplianceModule(complianceToken);
      setShowComplianceModal(false);
    } catch (err) {
      alert(
        err instanceof Error
          ? err.message
          : "Failed to detach compliance module"
      );
    } finally {
      setComplianceLoading(false);
    }
  };

  // ─── Handlers — privacy ───────────────────────────────────────────────────
  const openPrivacyModal = (token: SssToken) => {
    setPrivacyToken(token);
    setAllowlistAuthority("");
    setShowPrivacyModal(true);
  };

  const handleAttachPrivacy = async () => {
    if (!privacyToken || !allowlistAuthority) return;
    setPrivacyLoading(true);
    try {
      await attachPrivacyModule(privacyToken, allowlistAuthority);
      setShowPrivacyModal(false);
      setAllowlistAuthority("");
    } catch (err) {
      alert(
        err instanceof Error ? err.message : "Failed to attach privacy module"
      );
    } finally {
      setPrivacyLoading(false);
    }
  };

  const handleDetachPrivacy = async () => {
    if (!privacyToken) return;
    setPrivacyLoading(true);
    try {
      await detachPrivacyModule(privacyToken);
      setShowPrivacyModal(false);
    } catch (err) {
      alert(
        err instanceof Error ? err.message : "Failed to detach privacy module"
      );
    } finally {
      setPrivacyLoading(false);
    }
  };

  // ─── Handler — pause ──────────────────────────────────────────────────────
  const handleTogglePause = async (token: SssToken) => {
    setPausingToken(token.mint);
    try {
      await pauseToken(token, !token.paused);
    } catch (err) {
      alert(
        err instanceof Error ? err.message : "Failed to pause/unpause token"
      );
    } finally {
      setPausingToken(null);
    }
  };

  if (!connected) {
    return (
      <section className="osint-card p-6 rounded-md relative overflow-hidden">
        <div className="absolute top-0 right-0 p-2 opacity-10 font-mono text-xs">
          SYS_LOG_09X
        </div>
        <div className="flex justify-between items-end mb-6">
          <div>
            <h2 className="text-sm font-mono text-[#25d1f4] mb-1 uppercase tracking-widest flex items-center gap-2">
              <span className="w-2 h-2 bg-[#25d1f4] animate-pulse"></span>
              My Assets
            </h2>
            <p className="text-2xl font-bold">Token Management Console</p>
          </div>
        </div>
        <div className="text-center py-8 text-slate-500">
          Connect wallet to view tokens
        </div>
      </section>
    );
  }

  return (
    <>
      <section className="osint-card p-6 rounded-md relative overflow-hidden">
        <div className="absolute top-0 right-0 p-2 opacity-10 font-mono text-xs">
          SYS_LOG_09X
        </div>

        <div className="flex justify-between items-end mb-6">
          <div>
            <h2 className="text-sm font-mono text-[#25d1f4] mb-1 uppercase tracking-widest flex items-center gap-2">
              <span className="w-2 h-2 bg-[#25d1f4] animate-pulse"></span>
              My Assets
            </h2>
            <p className="text-2xl font-bold">Token Management Console</p>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => refreshTokens(true)}
              className="px-4 py-2 text-[10px] border border-white/20 text-slate-400 hover:border-[#25d1f4] hover:text-[#25d1f4] transition-colors uppercase"
            >
              Refresh
            </button>
            <button
              onClick={() => {
                setTokenName("");
                setTokenSymbol("");
                setShowCreateModal(true);
              }}
              className="bg-[#25d1f4] text-black px-4 py-2 text-xs font-bold uppercase hover:bg-white transition-colors"
            >
              Create New Token
            </button>
          </div>
        </div>

        {isLoading ? (
          <div className="text-center py-8 text-slate-500">
            Loading tokens...
          </div>
        ) : tokens.length === 0 ? (
          <div className="text-center py-8 text-slate-500">
            No SSS tokens found for this wallet
          </div>
        ) : (
          <div className="overflow-x-auto">
            {/* Fixed header */}
            <table className="w-full text-left border-collapse">
              <thead className="text-[10px] uppercase font-mono text-slate-500 border-b border-white/5">
                <tr>
                  <th className="pb-3 px-4">Token</th>
                  <th className="pb-3 px-4">Supply</th>
                  <th className="pb-3 px-4">Tier</th>
                  <th className="pb-3 px-4 text-center" colSpan={4}>
                    Modules &amp; Actions
                  </th>
                  <th className="pb-3 px-4 text-right">State</th>
                </tr>
              </thead>
            </table>

            {/* Scrollable rows */}
            <div
              className="overflow-y-auto no-scrollbar"
              style={{ maxHeight: "224px" }}
            >
              <table className="w-full text-left border-collapse">
                <tbody className="text-sm">
                  {tokens.map((token, i) => {
                    const tier = getTier(token);
                    return (
                      <tr
                        key={i}
                        className="border-b border-white/5 hover:bg-white/5 transition-colors"
                      >
                        {/* Token name */}
                        <td className="py-4 px-4 font-bold text-[#25d1f4] w-[160px]">
                          {token.name
                            ? `${token.name} (${token.symbol})`
                            : `${token.mint.slice(0, 8)}...${token.mint.slice(
                                -4
                              )}`}
                        </td>

                        {/* Supply */}
                        <td className="py-4 px-4 font-mono text-sm">
                          {formatSupply(token.supply, token.decimals)}
                        </td>

                        {/* Tier badge */}
                        <td className="py-4 px-4">
                          <span
                            className={`text-[10px] px-2 py-0.5 rounded-full uppercase font-mono ${tier.color}`}
                          >
                            {tier.label}
                          </span>
                        </td>

                        {/* Compliance module button */}
                        <td className="py-4 px-1">
                          <button
                            onClick={() => openComplianceModal(token)}
                            className={`w-full text-[10px] border py-1 px-2 transition-colors uppercase ${
                              token.complianceAttached
                                ? "border-blue-500/40 text-blue-400 hover:bg-blue-500/20"
                                : "border-white/20 text-slate-400 hover:border-[#25d1f4] hover:text-[#25d1f4]"
                            }`}
                          >
                            {token.complianceAttached
                              ? "Compliance ✓"
                              : "Compliance"}
                          </button>
                        </td>

                        {/* Privacy module button */}
                        <td className="py-4 px-1">
                          <button
                            onClick={() => openPrivacyModal(token)}
                            className={`w-full text-[10px] border py-1 px-2 transition-colors uppercase ${
                              token.privacyAttached
                                ? "border-purple-500/40 text-purple-400 hover:bg-purple-500/20"
                                : "border-white/20 text-slate-400 hover:border-[#25d1f4] hover:text-[#25d1f4]"
                            }`}
                          >
                            {token.privacyAttached ? "Privacy ✓" : "Privacy"}
                          </button>
                        </td>

                        {/* Pause / Unpause */}
                        <td className="py-4 px-1">
                          <button
                            onClick={() => handleTogglePause(token)}
                            disabled={pausingToken === token.mint}
                            className={`w-full text-[10px] border py-1 px-2 transition-all uppercase disabled:opacity-50 ${
                              token.paused
                                ? "border-green-500/40 text-green-400 hover:bg-green-500/20"
                                : "border-yellow-500/40 text-yellow-400 hover:bg-yellow-500/20"
                            }`}
                          >
                            {pausingToken === token.mint
                              ? "..."
                              : token.paused
                              ? "Unpause"
                              : "Pause"}
                          </button>
                        </td>

                        {/* Select */}
                        <td className="py-4 px-1">
                          <button
                            onClick={() => setSelectedToken(token)}
                            className={`w-full text-[10px] border py-1 px-2 transition-all uppercase ${
                              selectedToken?.mint === token.mint
                                ? "bg-[#25d1f4] text-black border-[#25d1f4]"
                                : "bg-[#25d1f4]/20 text-[#25d1f4] border-[#25d1f4]/40 hover:bg-[#25d1f4] hover:text-black"
                            }`}
                          >
                            {selectedToken?.mint === token.mint
                              ? "Selected"
                              : "Select"}
                          </button>
                        </td>

                        {/* Active / Paused state */}
                        <td className="py-4 px-4 text-right">
                          <span
                            className={`text-[10px] px-2 py-0.5 rounded-full uppercase ${
                              token.paused
                                ? "bg-yellow-500/20 text-yellow-400"
                                : "bg-green-500/20 text-green-400"
                            }`}
                          >
                            {token.paused ? "Paused" : "Active"}
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {tokens.length > 4 && (
          <div className="mt-4 text-center">
            <p className="text-[10px] font-mono text-slate-500 animate-bounce">
              ↓ Scroll to show other tokens ↓
            </p>
          </div>
        )}
      </section>

      {/* ── Create Token Modal ─────────────────────────────────────────────── */}
      {showCreateModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-2 text-[#25d1f4]">
              Create New Token
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              Creates an SSS-1 token. Attach the Compliance Module afterwards to
              upgrade to SSS-2, or both modules for SSS-3.
            </p>
            <div className="space-y-4">
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Token Name
                </label>
                <input
                  type="text"
                  value={tokenName}
                  onChange={(e) => setTokenName(e.target.value)}
                  placeholder="My Stablecoin"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Symbol
                </label>
                <input
                  type="text"
                  value={tokenSymbol}
                  onChange={(e) => setTokenSymbol(e.target.value.toUpperCase())}
                  placeholder="MYS"
                  maxLength={10}
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>
              <button
                onClick={handleCreateToken}
                disabled={creating || !tokenName || !tokenSymbol}
                className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {creating ? "Creating..." : "Create Token"}
              </button>
              <button
                onClick={() => {
                  setShowCreateModal(false);
                  setTokenName("");
                  setTokenSymbol("");
                }}
                className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ── Create Success Modal ───────────────────────────────────────────── */}
      {showSuccessModal && createdToken && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4 text-center">
            <div className="w-16 h-16 bg-green-500/20 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg
                className="w-8 h-8 text-green-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            </div>
            <h3 className="text-lg font-bold mb-4 text-green-400">
              Token Created!
            </h3>
            <div className="space-y-2 text-sm">
              <div className="bg-black/30 rounded p-3">
                <div className="text-slate-400 text-xs uppercase">
                  Token Name
                </div>
                <div className="text-[#25d1f4] font-bold">
                  {createdToken.name}
                </div>
              </div>
              <div className="bg-black/30 rounded p-3">
                <div className="text-slate-400 text-xs uppercase">Symbol</div>
                <div className="text-[#25d1f4] font-bold">
                  {createdToken.symbol}
                </div>
              </div>
              <div className="bg-black/30 rounded p-3">
                <div className="text-slate-400 text-xs uppercase">
                  Mint Address
                </div>
                <div className="text-white font-mono text-xs break-all">
                  {createdToken.mint}
                </div>
              </div>
            </div>
            <button
              onClick={() => setShowSuccessModal(false)}
              className="mt-4 w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}

      {/* ── Compliance Module Modal ────────────────────────────────────────── */}
      {showComplianceModal && complianceToken && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-2 text-[#25d1f4]">
              Compliance Module
            </h3>
            <p className="text-sm text-slate-400 mb-1">
              {complianceToken.name ||
                `${complianceToken.mint.slice(
                  0,
                  8
                )}...${complianceToken.mint.slice(-4)}`}
            </p>
            <div
              className={`inline-flex items-center gap-1.5 text-[10px] px-2 py-0.5 rounded-full mb-6 ${
                complianceToken.complianceAttached
                  ? "bg-blue-500/20 text-blue-400"
                  : "bg-slate-500/20 text-slate-400"
              }`}
            >
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  complianceToken.complianceAttached
                    ? "bg-blue-400"
                    : "bg-slate-500"
                }`}
              />
              {complianceToken.complianceAttached ? "Attached" : "Not attached"}
            </div>

            {complianceToken.complianceAttached ? (
              <div className="space-y-3">
                <p className="text-xs text-slate-400">
                  Detaching removes blacklist enforcement from all transfers.
                  Existing blacklist entries are not deleted.
                </p>
                <button
                  onClick={handleDetachCompliance}
                  disabled={complianceLoading}
                  className="w-full py-3 border border-red-500/40 text-red-400 hover:bg-red-500/20 text-sm font-bold uppercase transition-colors disabled:opacity-50"
                >
                  {complianceLoading
                    ? "Detaching..."
                    : "Detach Compliance Module"}
                </button>
                <button
                  onClick={() => setShowComplianceModal(false)}
                  className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors text-sm"
                >
                  Cancel
                </button>
              </div>
            ) : (
              <div className="space-y-4">
                <p className="text-xs text-slate-400">
                  Attaching enables blacklist enforcement on transfers and
                  upgrades this token to SSS-2.
                </p>
                <div>
                  <label className="block text-xs text-slate-400 mb-1 uppercase">
                    Blacklister Address
                  </label>
                  <input
                    type="text"
                    value={blacklisterAddress}
                    onChange={(e) => setBlacklisterAddress(e.target.value)}
                    placeholder="Wallet allowed to blacklist addresses"
                    className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none text-sm"
                  />
                </div>
                <button
                  onClick={handleAttachCompliance}
                  disabled={complianceLoading || !blacklisterAddress}
                  className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {complianceLoading
                    ? "Attaching..."
                    : "Attach Compliance Module"}
                </button>
                <button
                  onClick={() => {
                    setShowComplianceModal(false);
                    setBlacklisterAddress("");
                  }}
                  className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors text-sm"
                >
                  Cancel
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      {/* ── Privacy Module Modal ───────────────────────────────────────────── */}
      {showPrivacyModal && privacyToken && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-2 text-[#25d1f4]">
              Privacy Module
            </h3>
            <p className="text-sm text-slate-400 mb-1">
              {privacyToken.name ||
                `${privacyToken.mint.slice(0, 8)}...${privacyToken.mint.slice(
                  -4
                )}`}
            </p>
            <div
              className={`inline-flex items-center gap-1.5 text-[10px] px-2 py-0.5 rounded-full mb-6 ${
                privacyToken.privacyAttached
                  ? "bg-purple-500/20 text-purple-400"
                  : "bg-slate-500/20 text-slate-400"
              }`}
            >
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  privacyToken.privacyAttached
                    ? "bg-purple-400"
                    : "bg-slate-500"
                }`}
              />
              {privacyToken.privacyAttached ? "Attached" : "Not attached"}
            </div>

            {privacyToken.privacyAttached ? (
              <div className="space-y-3">
                <p className="text-xs text-slate-400">
                  Detaching removes allowlist enforcement. All addresses will be
                  able to transfer freely.
                </p>
                <button
                  onClick={handleDetachPrivacy}
                  disabled={privacyLoading}
                  className="w-full py-3 border border-red-500/40 text-red-400 hover:bg-red-500/20 text-sm font-bold uppercase transition-colors disabled:opacity-50"
                >
                  {privacyLoading ? "Detaching..." : "Detach Privacy Module"}
                </button>
                <button
                  onClick={() => setShowPrivacyModal(false)}
                  className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors text-sm"
                >
                  Cancel
                </button>
              </div>
            ) : (
              <div className="space-y-4">
                <p className="text-xs text-slate-400">
                  Attaching enables allowlist-only transfers. Only wallets on
                  the allowlist can send or receive.
                  {privacyToken.complianceAttached
                    ? " Combined with Compliance, this upgrades the token to SSS-3."
                    : ""}
                </p>
                <div>
                  <label className="block text-xs text-slate-400 mb-1 uppercase">
                    Allowlist Authority
                  </label>
                  <input
                    type="text"
                    value={allowlistAuthority}
                    onChange={(e) => setAllowlistAuthority(e.target.value)}
                    placeholder="Wallet that manages the allowlist"
                    className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none text-sm"
                  />
                </div>
                <button
                  onClick={handleAttachPrivacy}
                  disabled={privacyLoading || !allowlistAuthority}
                  className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {privacyLoading ? "Attaching..." : "Attach Privacy Module"}
                </button>
                <button
                  onClick={() => {
                    setShowPrivacyModal(false);
                    setAllowlistAuthority("");
                  }}
                  className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors text-sm"
                >
                  Cancel
                </button>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
