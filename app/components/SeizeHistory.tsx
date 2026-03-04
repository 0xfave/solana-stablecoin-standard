"use client";

import { useState, useEffect } from "react";
import { useSolana, SssToken } from "@/lib/useSolana";

interface SeizeHistoryProps {
  token: SssToken | null;
}

interface SeizeRecord {
  from: string;
  fromFull: string;
  to: string;
  amount: string;
  txn: string;
  time: string;
}

export default function SeizeHistory({ token }: SeizeHistoryProps) {
  const { fetchSeizeHistory, seize } = useSolana();
  const [history, setHistory] = useState<SeizeRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [showModal, setShowModal] = useState(false);
  const [fromAddress, setFromAddress] = useState("");
  const [toAddress, setToAddress] = useState("");
  const [amount, setAmount] = useState("");
  const [seizing, setSeizing] = useState(false);
  const [showSuccess, setShowSuccess] = useState(false);

  const tokenLabel = token
    ? token.name || token.symbol || `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`
    : "Select a token";

  useEffect(() => {
    if (token) {
      setLoading(true);
      fetchSeizeHistory(token).then((data) => {
        setHistory(data);
        setLoading(false);
      });
    } else {
      setHistory([]);
    }
  }, [token, fetchSeizeHistory]);

  const handleSeize = async () => {
    if (!token || !fromAddress || !toAddress || !amount) return;
    setSeizing(true);
    try {
      const amountInSmallestUnits = Math.floor(parseFloat(amount) * Math.pow(10, token.decimals));
      await seize(token, fromAddress, toAddress, amountInSmallestUnits);
      const updatedHistory = await fetchSeizeHistory(token);
      setHistory(updatedHistory);
      setShowModal(false);
      setFromAddress("");
      setToAddress("");
      setAmount("");
      setShowSuccess(true);
    } catch (err) {
      console.error("Error seizing tokens:", err);
      alert(err instanceof Error ? err.message : "Failed to seize tokens");
    } finally {
      setSeizing(false);
    }
  };

  return (
    <>
      <section className="osint-card flex flex-col h-[400px]">
        <div className="p-4 border-b border-white/10 flex justify-between items-center">
          <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">Seize History</h3>
          <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
        </div>
        <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
          {!token ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Select a token to view seize history
            </div>
          ) : loading ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Loading...
            </div>
          ) : history.length === 0 ? (
            <div className="text-center text-slate-500 text-sm py-8">
              No seized funds for this token
            </div>
          ) : (
            history.map((item, i) => (
              <div key={i} className="bg-[#141417] p-3 rounded border border-white/5">
                <div className="flex justify-between items-start mb-2">
                  <span className="text-[10px] font-mono text-slate-400">From: {item.from}</span>
                  <span className="text-xs font-mono text-[#25d1f4]">{item.amount} SSS</span>
                </div>
                <div className="text-[10px] opacity-50 font-mono mb-1">To: {item.to.slice(0, 4)}...{item.to.slice(-4)}</div>
                <div className="text-[10px] opacity-30 font-mono">TXN: {item.txn}</div>
              </div>
            ))
          )}
        </div>
        <div className="p-3 border-t border-white/10">
          <button 
            onClick={() => setShowModal(true)}
            disabled={!token}
            className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-[#25d1f4] hover:text-black transition-colors disabled:opacity-50"
          >
            address | seize
          </button>
        </div>
      </section>

      {showModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px]">
            <h3 className="text-sm font-bold uppercase tracking-widest text-[#25d1f4] mb-4">
              Seize Tokens
            </h3>
            <input
              type="text"
              value={fromAddress}
              onChange={(e) => setFromAddress(e.target.value)}
              placeholder="From wallet address (blacklisted)"
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-3 focus:border-[#25d1f4] outline-none"
            />
            <input
              type="text"
              value={toAddress}
              onChange={(e) => setToAddress(e.target.value)}
              placeholder="To wallet address (destination)"
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-3 focus:border-[#25d1f4] outline-none"
            />
            <input
              type="number"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              placeholder={`Amount (${token.symbol || 'SSS'})`}
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-4 focus:border-[#25d1f4] outline-none"
            />
            <div className="flex gap-3">
              <button
                onClick={() => {
                  setShowModal(false);
                  setFromAddress("");
                  setToAddress("");
                  setAmount("");
                }}
                className="flex-1 py-2 bg-white/5 text-xs uppercase font-bold hover:bg-white/10 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleSeize}
                disabled={seizing || !fromAddress || !toAddress || !amount}
                className="flex-1 py-2 bg-[#25d1f4] text-black text-xs uppercase font-bold hover:bg-[#25d1f4]/90 transition-colors disabled:opacity-50"
              >
                {seizing ? "Seizing..." : "Seize"}
              </button>
            </div>
          </div>
        </div>
      )}

      {showSuccess && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px] text-center">
            <div className="w-12 h-12 bg-[#25d1f4]/20 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg className="w-6 h-6 text-[#25d1f4]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <h3 className="text-sm font-bold uppercase tracking-widest text-white mb-2">
              Tokens Seized!
            </h3>
            <p className="text-xs text-slate-400 mb-6">
              The tokens have been successfully seized.
            </p>
            <button
              onClick={() => setShowSuccess(false)}
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
