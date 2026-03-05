"use client";

import { useState, useEffect } from "react";
import { useSolana, SssToken } from "@/lib/useSolana";

interface BurnHistoryProps {
  token: SssToken | null;
}

interface BurnRecord {
  amount: string;
  from: string;
  txn: string;
  time: string;
}

export default function BurnHistory({ token }: BurnHistoryProps) {
  const { fetchBurnHistory, burnTokens } = useSolana();
  const [history, setHistory] = useState<BurnRecord[]>([]);
  const [loading, setLoading] = useState(false);

  const [showModal, setShowModal] = useState(false);
  const [burnAmount, setBurnAmount] = useState("");
  const [burnFromAddress, setBurnFromAddress] = useState("");
  const [burning, setBurning] = useState(false);
  const [showSuccess, setShowSuccess] = useState(false);

  const tokenLabel = token
    ? token.name ||
      token.symbol ||
      `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`
    : "Select a token";

  useEffect(() => {
    if (token) {
      setLoading(true);
      fetchBurnHistory(token).then((data) => {
        setHistory(data);
        setLoading(false);
      });
    } else {
      setHistory([]);
    }
  }, [token, fetchBurnHistory]);

  const handleBurn = async () => {
    if (!token || !burnAmount || !burnFromAddress) return;
    setBurning(true);
    try {
      const amountInSmallest = Math.floor(
        parseFloat(burnAmount) * Math.pow(10, token.decimals)
      );
      await burnTokens(token, burnFromAddress, amountInSmallest);

      // Wait for RPC to index then refresh history
      await new Promise((r) => setTimeout(r, 2000));
      const updated = await fetchBurnHistory(token);
      setHistory(updated);

      setShowModal(false);
      setBurnAmount("");
      setBurnFromAddress("");
      setShowSuccess(true);
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to burn tokens");
    } finally {
      setBurning(false);
    }
  };

  return (
    <>
      <section className="osint-card flex flex-col h-[400px]">
        <div className="p-4 border-b border-white/10 flex justify-between items-center">
          <h3 className="text-xs font-bold uppercase tracking-widest text-orange-500">
            Burn History
          </h3>
          <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
        </div>

        <div
          className="flex-1 overflow-y-auto p-4 space-y-3"
          style={{ scrollbarWidth: "none", msOverflowStyle: "none" }}
        >
          {!token ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Select a token to view burn history
            </div>
          ) : loading ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Loading...
            </div>
          ) : history.length === 0 ? (
            <div className="text-center text-slate-500 text-sm py-8">
              No burn history for this token
            </div>
          ) : (
            history.map((item, i) => (
              <div
                key={i}
                className={`bg-[#141417] p-3 rounded border-l-2 ${
                  i === 0 ? "border-orange-500" : "border-orange-500/40"
                }`}
              >
                <div className="flex justify-between items-start mb-2">
                  <span className="text-[10px] font-mono text-slate-400 truncate">
                    From: {item.from}
                  </span>
                  <span className="text-xs font-mono text-orange-400 ml-2 whitespace-nowrap">
                    {item.amount}
                  </span>
                </div>
                <div className="flex justify-between items-center text-[10px] opacity-50 font-mono">
                  <span>TXN: {item.txn}</span>
                  <span>{item.time}</span>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="p-3 border-t border-white/10">
          <button
            onClick={() => setShowModal(true)}
            disabled={!token}
            className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-orange-500 hover:text-black transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Burn Tokens
          </button>
        </div>
      </section>

      {/* Burn Modal */}
      {showModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-2 text-orange-500">
              Burn Tokens
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              Permanently destroy tokens from a wallet. This action cannot be
              undone.
            </p>
            <div className="space-y-4">
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Wallet Address (token account owner)
                </label>
                <input
                  type="text"
                  value={burnFromAddress}
                  onChange={(e) => setBurnFromAddress(e.target.value)}
                  placeholder="Enter wallet address to burn from"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-orange-500 focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Amount ({token.symbol || "tokens"})
                </label>
                <input
                  type="number"
                  value={burnAmount}
                  onChange={(e) => setBurnAmount(e.target.value)}
                  placeholder="Amount to burn"
                  min="0"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-orange-500 focus:outline-none"
                />
              </div>

              {/* Warning box */}
              <div className="bg-orange-500/10 border border-orange-500/30 rounded p-3">
                <p className="text-[10px] text-orange-400 uppercase font-bold mb-1">
                  ⚠ Warning
                </p>
                <p className="text-[10px] text-slate-400">
                  Burned tokens are permanently removed from circulation. This
                  cannot be reversed.
                </p>
              </div>

              <button
                onClick={handleBurn}
                disabled={burning || !burnAmount || !burnFromAddress}
                className="w-full bg-orange-500 text-black py-3 font-bold uppercase hover:bg-orange-400 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {burning
                  ? "Burning..."
                  : `Burn ${burnAmount || "0"} ${token.symbol || "tokens"}`}
              </button>
              <button
                onClick={() => {
                  setShowModal(false);
                  setBurnAmount("");
                  setBurnFromAddress("");
                }}
                className="w-full py-2 border border-white/20 text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Success Modal */}
      {showSuccess && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-sm w-full mx-4 text-center">
            <div className="w-12 h-12 bg-orange-500/20 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg
                className="w-6 h-6 text-orange-500"
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
              Tokens Burned!
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              The tokens have been permanently removed from circulation.
            </p>
            <button
              onClick={() => setShowSuccess(false)}
              className="w-full bg-orange-500 text-black py-3 font-bold uppercase hover:bg-orange-400 transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}
    </>
  );
}
