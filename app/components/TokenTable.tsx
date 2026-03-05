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
    addMinter,
    addFreezer,
    pauseToken,
    selectedToken,
    setSelectedToken,
  } = useSolana();

  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showSuccessModal, setShowSuccessModal] = useState(false);
  const [showMinterSuccess, setShowMinterSuccess] = useState(false);
  const [createdToken, setCreatedToken] = useState<{
    name: string;
    symbol: string;
    mint: string;
  } | null>(null);
  const [creating, setCreating] = useState(false);
  const [tokenName, setTokenName] = useState("");
  const [tokenSymbol, setTokenSymbol] = useState("");
  const [selectedPreset, setSelectedPreset] = useState<number | null>(null);

  const [showAddMinterModal, setShowAddMinterModal] = useState(false);
  const [minterAddress, setMinterAddress] = useState("");
  const [addingMinter, setAddingMinter] = useState(false);

  const [showAddFreezerModal, setShowAddFreezerModal] = useState(false);
  const [freezerAddress, setFreezerAddress] = useState("");
  const [addingFreezer, setAddingFreezer] = useState(false);
  const [showFreezerSuccess, setShowFreezerSuccess] = useState(false);

  const [pausingToken, setPausingToken] = useState<string | null>(null);

  const formatSupply = (supply: string, decimals: number) => {
    try {
      const num = Number(supply) / Math.pow(10, decimals);
      return num.toLocaleString(undefined, {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
      });
    } catch {
      return supply;
    }
  };

  const handleCreateToken = async () => {
    if (!tokenName || !tokenSymbol || selectedPreset === null) {
      alert("Please fill in all fields");
      return;
    }
    setCreating(true);
    try {
      const result = await createToken(
        selectedPreset,
        6,
        tokenName,
        tokenSymbol
      );
      setShowCreateModal(false);
      setCreatedToken({
        name: tokenName,
        symbol: tokenSymbol,
        mint: result.mintAddress.toString(),
      });
      setShowSuccessModal(true);
      setTokenName("");
      setTokenSymbol("");
      setSelectedPreset(null);
    } catch (err) {
      console.error("Error creating token:", err);
      alert(err instanceof Error ? err.message : "Failed to create token");
    } finally {
      setCreating(false);
    }
  };

  const handleAddMinter = async () => {
    if (!selectedToken || !minterAddress) {
      alert("Please enter a minter address");
      return;
    }
    setAddingMinter(true);
    try {
      await addMinter(selectedToken, minterAddress);
      setShowAddMinterModal(false);
      setSelectedToken(null);
      setMinterAddress("");
      setShowMinterSuccess(true);
    } catch (err) {
      console.error("Error adding minter:", err);
      alert(err instanceof Error ? err.message : "Failed to add minter");
    } finally {
      setAddingMinter(false);
    }
  };

  const handleAddFreezer = async () => {
    if (!selectedToken || !freezerAddress) {
      alert("Please enter a freezer address");
      return;
    }
    setAddingFreezer(true);
    try {
      await addFreezer(selectedToken, freezerAddress);
      setShowAddFreezerModal(false);
      setShowFreezerSuccess(true);
    } catch (err) {
      console.error("Error adding freezer:", err);
      alert(err instanceof Error ? err.message : "Failed to add freezer");
    } finally {
      setAddingFreezer(false);
    }
  };

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

  const openAddMinterModal = (token: SssToken) => {
    setSelectedToken(token);
    setMinterAddress("");
    setShowAddMinterModal(true);
  };

  const openAddFreezerModal = (token: SssToken) => {
    setSelectedToken(token);
    setFreezerAddress("");
    setShowAddFreezerModal(true);
  };

  const openModal = () => {
    setSelectedPreset(null);
    setTokenName("");
    setTokenSymbol("");
    setShowCreateModal(true);
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
              onClick={() => openModal()}
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
          // ✅ Fixed height showing max 4 tokens, scrollable
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse">
              <thead className="text-[10px] uppercase font-mono text-slate-500 border-b border-white/5">
                <tr>
                  <th className="pb-3 px-4">Token Address</th>
                  <th className="pb-3 px-4">Total Supply</th>
                  <th className="pb-3 px-4 text-center" colSpan={4}>
                    Management Operations
                  </th>
                  <th className="pb-3 px-4 text-right">State</th>
                </tr>
              </thead>
            </table>
            {/* ✅ Scrollable body capped at 4 rows */}
            <div className="overflow-y-auto no-scrollbar" style={{ maxHeight: "224px" }}>
              <table className="w-full text-left border-collapse">
                <tbody className="text-sm">
                  {tokens.map((token, i) => (
                    <tr
                      key={i}
                      className="border-b border-white/5 hover:bg-white/5 group transition-colors"
                    >
                      <td className="py-4 px-4 font-bold text-[#25d1f4] w-[160px]">
                        {token.name
                          ? `${token.name} (${token.symbol})`
                          : `${token.mint.slice(0, 8)}...${token.mint.slice(
                              -4
                            )}`}
                      </td>
                      <td className="py-4 px-4 font-mono">
                        {formatSupply(token.supply, token.decimals)}
                      </td>
                      <td className="py-4 px-1">
                        <button
                          onClick={() => openAddMinterModal(token)}
                          className="w-full text-[10px] border border-white/20 py-1 px-2 hover:border-[#25d1f4] transition-colors uppercase"
                        >
                          Add Minter
                        </button>
                      </td>
                      <td className="py-4 px-1">
                        <button
                          onClick={() => openAddFreezerModal(token)}
                          className="w-full text-[10px] border border-white/20 py-1 px-2 hover:border-[#25d1f4] transition-colors uppercase"
                        >
                          Add Freezer
                        </button>
                      </td>
                      <td className="py-4 px-1">
                        {/* ✅ Pause / Unpause button */}
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
                  ))}
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

      {/* Create Token Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-4 text-[#25d1f4]">
              Create New Token
            </h3>
            {selectedPreset === null ? (
              <>
                <p className="text-sm text-slate-400 mb-6">
                  Select a preset for your stablecoin:
                </p>
                <div className="space-y-3">
                  <button
                    onClick={() => setSelectedPreset(0)}
                    className="w-full p-4 border border-[#25d1f4]/40 bg-[#25d1f4]/10 hover:bg-[#25d1f4]/20 rounded-md text-left transition-colors"
                  >
                    <div className="font-bold text-[#25d1f4]">SSS-1</div>
                    <div className="text-xs text-slate-400">
                      Basic stablecoin - no compliance features
                    </div>
                  </button>
                  <button
                    onClick={() => setSelectedPreset(1)}
                    className="w-full p-4 border border-[#25d1f4]/40 bg-[#25d1f4]/10 hover:bg-[#25d1f4]/20 rounded-md text-left transition-colors"
                  >
                    <div className="font-bold text-[#25d1f4]">SSS-2</div>
                    <div className="text-xs text-slate-400">
                      Compliant stablecoin - blacklist, freeze, seize
                    </div>
                  </button>
                </div>
                <button
                  onClick={() => setShowCreateModal(false)}
                  className="mt-4 w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors"
                >
                  Cancel
                </button>
              </>
            ) : (
              <>
                <p className="text-sm text-slate-400 mb-6">
                  Configure your stablecoin:
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
                      onChange={(e) =>
                        setTokenSymbol(e.target.value.toUpperCase())
                      }
                      placeholder="MYS"
                      maxLength={10}
                      className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-slate-400 mb-1 uppercase">
                      Preset
                    </label>
                    <div className="text-sm text-[#25d1f4]">
                      {selectedPreset === 0
                        ? "SSS-1 (Basic)"
                        : "SSS-2 (Compliant)"}
                    </div>
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
                      setSelectedPreset(null);
                      setTokenName("");
                      setTokenSymbol("");
                    }}
                    className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors"
                  >
                    Back
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {/* Success Modal */}
      {showSuccessModal && createdToken && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <div className="text-center">
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
              <h3 className="text-lg font-bold mb-2 text-green-400">
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
        </div>
      )}

      {/* Add Minter Modal */}
      {showAddMinterModal && selectedToken && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-4 text-[#25d1f4]">
              Add Minter
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              Add a new minter address for token:{" "}
              {selectedToken.mint.slice(0, 8)}...{selectedToken.mint.slice(-4)}
            </p>
            <div className="space-y-4">
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Minter Address
                </label>
                <input
                  type="text"
                  value={minterAddress}
                  onChange={(e) => setMinterAddress(e.target.value)}
                  placeholder="Enter minter wallet address"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>
              <button
                onClick={handleAddMinter}
                disabled={addingMinter || !minterAddress}
                className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {addingMinter ? "Adding..." : "Add Minter"}
              </button>
              <button
                onClick={() => {
                  setShowAddMinterModal(false);
                  setSelectedToken(null);
                  setMinterAddress("");
                }}
                className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Add Freezer Modal */}
      {showAddFreezerModal && selectedToken && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-4 text-[#25d1f4]">
              Add Freezer
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              Add a new freezer address for token:{" "}
              {selectedToken.mint.slice(0, 8)}...{selectedToken.mint.slice(-4)}
            </p>
            <div className="space-y-4">
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Freezer Address
                </label>
                <input
                  type="text"
                  value={freezerAddress}
                  onChange={(e) => setFreezerAddress(e.target.value)}
                  placeholder="Enter freezer wallet address"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>
              <button
                onClick={handleAddFreezer}
                disabled={addingFreezer || !freezerAddress}
                className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {addingFreezer ? "Adding..." : "Add Freezer"}
              </button>
              <button
                onClick={() => {
                  setShowAddFreezerModal(false);
                  setSelectedToken(null);
                  setFreezerAddress("");
                }}
                className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Freezer Success Modal */}
      {showFreezerSuccess && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-sm w-full mx-4 text-center">
            <div className="w-12 h-12 bg-[#25d1f4]/20 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg
                className="w-6 h-6 text-[#25d1f4]"
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
            <h3 className="text-lg font-bold mb-2 text-white">
              Freezer Added!
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              The freezer address has been successfully added.
            </p>
            <button
              onClick={() => {
                setShowFreezerSuccess(false);
                setSelectedToken(null);
                setFreezerAddress("");
              }}
              className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}

      {/* Minter Success Modal */}
      {showMinterSuccess && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-sm w-full mx-4 text-center">
            <div className="w-12 h-12 bg-[#25d1f4]/20 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg
                className="w-6 h-6 text-[#25d1f4]"
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
            <h3 className="text-lg font-bold mb-2 text-white">Minter Added!</h3>
            <p className="text-sm text-slate-400 mb-6">
              The minter address has been successfully added. They can now mint
              tokens.
            </p>
            <button
              onClick={() => {
                setShowMinterSuccess(false);
                setSelectedToken(null);
                setMinterAddress("");
              }}
              className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}
    </>
  );
}
