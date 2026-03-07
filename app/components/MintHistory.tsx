"use client";

import { useState, useEffect, useCallback } from "react";
import { SssToken, useSolana } from "@/lib/useSolana";

interface MintHistoryProps {
  token: SssToken | null;
}

export default function MintHistory({ token }: MintHistoryProps) {
  const { mint, fetchMintHistory, refreshTokens } = useSolana();

  const [showMintModal, setShowMintModal] = useState(false);
  const [recipientAddress, setRecipientAddress] = useState("");
  const [mintAmount, setMintAmount] = useState("");
  const [minting, setMinting] = useState(false);
  const [mintHistory, setMintHistory] = useState<
    { amount: string; to: string; txn: string; time: string }[]
  >([]);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [showSuccessModal, setShowSuccessModal] = useState(false);
  const [mintedToken, setMintedToken] = useState<{
    amount: string;
    recipient: string;
  } | null>(null);

  const refreshMintHistory = useCallback(async () => {
    if (token) {
      const history = await fetchMintHistory(token);
      setMintHistory(history);
    }
  }, [token, fetchMintHistory]);

  useEffect(() => {
    if (token) {
      setLoadingHistory(true);
      fetchMintHistory(token).then((history) => {
        setMintHistory(history);
        setLoadingHistory(false);
      });
    } else {
      setMintHistory([]);
    }
  }, [token, fetchMintHistory]);

  const tokenLabel = token
    ? token.name ||
      token.symbol ||
      `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`
    : "Select a token";

  const handleMint = async () => {
    if (!token || !recipientAddress || !mintAmount) {
      alert("Please fill in all fields");
      return;
    }
    setMinting(true);
    try {
      await mint(token, recipientAddress, parseFloat(mintAmount));
      setShowMintModal(false);
      setMintedToken({ amount: mintAmount, recipient: recipientAddress });
      setShowSuccessModal(true);
      setRecipientAddress("");
      setMintAmount("");
      await Promise.all([refreshMintHistory(), refreshTokens(true)]);
    } catch (err) {
      console.error("Error minting:", err);
      alert(err instanceof Error ? err.message : "Failed to mint tokens");
    } finally {
      setMinting(false);
    }
  };

  return (
    <>
      <section className="osint-card flex flex-col h-[400px]">
        <div className="p-4 border-b border-white/10 flex justify-between items-center">
          <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">
            Mint History
          </h3>
          <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
        </div>
        <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
          {!token ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Select a token to view mint history
            </div>
          ) : loadingHistory ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Loading...
            </div>
          ) : mintHistory.length === 0 ? (
            <div className="text-center text-slate-500 text-sm py-8">
              No mint history for this token
            </div>
          ) : (
            mintHistory.map((item, i) => (
              <div
                key={i}
                className={`bg-[#141417] p-3 rounded border-l-2 ${
                  i === 0 ? "border-[#25d1f4]" : "border-[#25d1f4]/40"
                }`}
              >
                <div className="flex justify-between items-start mb-2">
                  <span className="text-xs font-bold text-[#25d1f4]">
                    {tokenLabel}
                  </span>
                  <span className="text-xs font-mono">{item.amount}</span>
                </div>
                <div className="flex justify-between items-center text-[10px] opacity-50 font-mono">
                  <span>To: {item.to}</span>
                  <span>{item.time}</span>
                </div>
              </div>
            ))
          )}
        </div>
        <div className="p-3 border-t border-white/10">
          <button
            onClick={() => token && setShowMintModal(true)}
            disabled={!token}
            className="w-full py-2 bg-[#25d1f4] text-black text-[10px] uppercase font-bold hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Mint New Tokens
          </button>
        </div>
      </section>

      {/* Mint Modal */}
      {showMintModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-4 text-[#25d1f4]">
              Mint Tokens
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              Mint new tokens for:{" "}
              {token.name || token.symbol || token.mint.slice(0, 8)}...
              {token.mint.slice(-4)}
            </p>
            <div className="space-y-4">
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Recipient Address
                </label>
                <input
                  type="text"
                  value={recipientAddress}
                  onChange={(e) => setRecipientAddress(e.target.value)}
                  placeholder="Enter recipient wallet address"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Amount
                </label>
                <input
                  type="number"
                  value={mintAmount}
                  onChange={(e) => setMintAmount(e.target.value)}
                  placeholder="Enter amount to mint"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>
              <button
                onClick={handleMint}
                disabled={minting || !recipientAddress || !mintAmount}
                className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {minting ? "Minting..." : "Mint Tokens"}
              </button>
              <button
                onClick={() => {
                  setShowMintModal(false);
                  setRecipientAddress("");
                  setMintAmount("");
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
      {showSuccessModal && mintedToken && (
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
                Tokens Minted!
              </h3>
              <div className="space-y-2 text-sm">
                <div className="bg-black/30 rounded p-3">
                  <div className="text-slate-400 text-xs uppercase">Amount</div>
                  <div className="text-[#25d1f4] font-bold">
                    {mintedToken.amount}
                  </div>
                </div>
                <div className="bg-black/30 rounded p-3">
                  <div className="text-slate-400 text-xs uppercase">
                    Recipient
                  </div>
                  <div className="text-white font-mono text-xs break-all">
                    {mintedToken.recipient}
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
    </>
  );
}
