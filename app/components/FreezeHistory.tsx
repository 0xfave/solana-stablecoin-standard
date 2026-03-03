"use client";

import { useState, useEffect } from "react";
import { useSolana, SssToken } from "@/lib/useSolana";

interface FreezeHistoryProps {
  token: SssToken | null;
}

interface FreezeRecord {
  account: string;
  accountFull: string;
  action: "freeze" | "thaw";
  txn: string;
  time: string;
}

export default function FreezeHistory({ token }: FreezeHistoryProps) {
  const { fetchFreezeHistory, freeze, thaw } = useSolana();
  const [history, setHistory] = useState<FreezeRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [accountToFreeze, setAccountToFreeze] = useState("");
  const [showFreezeModal, setShowFreezeModal] = useState(false);
  const [freezing, setFreezing] = useState(false);
  const [showSuccessModal, setShowSuccessModal] = useState(false);

  const tokenLabel = token
    ? token.name ||
      token.symbol ||
      `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`
    : "Select a token";

  useEffect(() => {
    if (token) {
      setLoading(true);
      fetchFreezeHistory(token).then((data) => {
        setHistory(data);
        setLoading(false);
      });
    } else {
      setHistory([]);
    }
  }, [token, fetchFreezeHistory]);

  const handleFreeze = async () => {
    if (!token || !accountToFreeze) return;
    setFreezing(true);
    try {
      await freeze(token, accountToFreeze);
      const updatedHistory = await fetchFreezeHistory(token);
      setHistory(updatedHistory);
      setShowFreezeModal(false);
      setAccountToFreeze("");
      setShowSuccessModal(true);
    } catch (err) {
      console.error("Error freezing account:", err);
      alert(err instanceof Error ? err.message : "Failed to freeze account");
    } finally {
      setFreezing(false);
    }
  };

  const handleThaw = async (accountFull: string) => {
    if (!token) return;
    setFreezing(true);
    try {
      await thaw(token, accountFull);
      const updatedHistory = await fetchFreezeHistory(token);
      setHistory(updatedHistory);
    } catch (err) {
      console.error("Error thawing account:", err);
      alert(err instanceof Error ? err.message : "Failed to thaw account");
    } finally {
      setFreezing(false);
    }
  };

  return (
    <>
      <section className="osint-card flex flex-col h-[400px]">
        <div className="p-4 border-b border-white/10 flex justify-between items-center">
          <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">
            Freeze History
          </h3>
          <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
        </div>
        <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
          {!token ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Select a token to view freeze history
            </div>
          ) : loading ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Loading...
            </div>
          ) : history.length === 0 ? (
            <div className="text-center text-slate-500 text-sm py-8">
              No freeze history for this token
            </div>
          ) : (
            history.map((record, i) => (
              <div
                key={i}
                className="bg-[#141417] p-3 rounded flex justify-between items-center border border-white/5"
              >
                <div className="flex flex-col">
                  <span className="text-xs font-bold">{record.account}</span>
                  <span className="text-[10px] opacity-50">{record.time}</span>
                </div>
                <button
                  onClick={() => handleThaw(record.accountFull)}
                  disabled={freezing}
                  className="text-[10px] px-3 py-1 bg-[#25d1f4]/10 border border-[#25d1f4]/40 text-[#25d1f4] hover:bg-[#25d1f4] hover:text-black transition-colors uppercase disabled:opacity-50"
                >
                  Thaw
                </button>
              </div>
            ))
          )}
        </div>
        <div className="p-3 border-t border-white/10">
          <button
            onClick={() => setShowFreezeModal(true)}
            disabled={!token}
            className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-[#25d1f4] hover:text-black transition-colors disabled:opacity-50"
          >
            Freeze Account
          </button>
        </div>
      </section>

      {showFreezeModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px]">
            <h3 className="text-sm font-bold uppercase tracking-widest text-[#25d1f4] mb-4">
              Freeze Account
            </h3>
            <input
              type="text"
              value={accountToFreeze}
              onChange={(e) => setAccountToFreeze(e.target.value)}
              placeholder="Enter account address to freeze"
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-4 focus:border-[#25d1f4] outline-none"
            />
            <div className="flex gap-3">
              <button
                onClick={() => {
                  setShowFreezeModal(false);
                  setAccountToFreeze("");
                }}
                className="flex-1 py-2 bg-white/5 text-xs uppercase font-bold hover:bg-white/10 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleFreeze}
                disabled={freezing || !accountToFreeze}
                className="flex-1 py-2 bg-[#25d1f4] text-black text-xs uppercase font-bold hover:bg-[#25d1f4]/90 transition-colors disabled:opacity-50"
              >
                {freezing ? "Freezing..." : "Freeze"}
              </button>
            </div>
          </div>
        </div>
      )}

      {showSuccessModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px] text-center">
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
            <h3 className="text-sm font-bold uppercase tracking-widest text-white mb-2">
              Account Frozen!
            </h3>
            <p className="text-xs text-slate-400 mb-6">
              The account has been successfully frozen.
            </p>
            <button
              onClick={() => setShowSuccessModal(false)}
              className="w-full py-2 bg-[#25d1f4] text-black text-xs uppercase font-bold hover:bg-[#25d1f4]/90 transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}
    </>
  );
}
