"use client";

import { useState, useEffect } from "react";
import { useSolana, SssToken } from "@/lib/useSolana";

interface BlacklistPanelProps {
  token: SssToken | null;
}

interface BlacklistEntry {
  address: string;
  addressFull: string;
  reason?: string;
  txn: string;
  time: string;
}

export default function BlacklistPanel({ token }: BlacklistPanelProps) {
  const { fetchBlacklistEntries, blacklistAdd, blacklistRemove } = useSolana();
  const [entries, setEntries] = useState<BlacklistEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [showAddModal, setShowAddModal] = useState(false);
  const [showRemoveModal, setShowRemoveModal] = useState(false);
  const [addressToBlacklist, setAddressToBlacklist] = useState("");
  const [reason, setReason] = useState("");
  const [adding, setAdding] = useState(false);
  const [showAddSuccess, setShowAddSuccess] = useState(false);
  const [addressToRemove, setAddressToRemove] = useState("");
  const [removing, setRemoving] = useState(false);
  const [showRemoveSuccess, setShowRemoveSuccess] = useState(false);
  const [showReasonModal, setShowReasonModal] = useState(false);
  const [selectedReason, setSelectedReason] = useState<string>("");

  const tokenLabel = token
    ? token.name ||
      token.symbol ||
      `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`
    : "Select a token";

  useEffect(() => {
    if (token) {
      setLoading(true);
      fetchBlacklistEntries(token).then((data) => {
        setEntries(data);
        setLoading(false);
      });
    } else {
      setEntries([]);
    }
  }, [token, fetchBlacklistEntries]);

  const handleAddBlacklist = async () => {
    if (!token || !addressToBlacklist) return;

    // Check if already blacklisted
    const alreadyBlacklisted = entries.some(
      (e) => e.addressFull.toLowerCase() === addressToBlacklist.toLowerCase()
    );
    if (alreadyBlacklisted) {
      alert("This address is already blacklisted.");
      return;
    }

    setAdding(true);
    try {
      await blacklistAdd(
        token,
        addressToBlacklist,
        reason || "No reason provided"
      );
      const updatedEntries = await fetchBlacklistEntries(token);
      setEntries(updatedEntries);
      setShowAddModal(false);
      setAddressToBlacklist("");
      setReason("");
      setShowAddSuccess(true);
    } catch (err) {
      console.error("Error adding to blacklist:", err);
      alert(err instanceof Error ? err.message : "Failed to add to blacklist");
    } finally {
      setAdding(false);
    }
  };

  const handleRemoveBlacklist = async () => {
    if (!token || !addressToRemove) return;
    setRemoving(true);
    try {
      await blacklistRemove(token, addressToRemove);
      const updatedEntries = await fetchBlacklistEntries(token);
      setEntries(updatedEntries);
      setShowRemoveModal(false);
      setAddressToRemove("");
      setShowRemoveSuccess(true);
    } catch (err) {
      console.error("Error removing from blacklist:", err);
      alert(
        err instanceof Error ? err.message : "Failed to remove from blacklist"
      );
    } finally {
      setRemoving(false);
    }
  };

  return (
    <>
      <section className="osint-card flex flex-col h-[400px]">
        <div className="p-4 border-b border-white/10 flex justify-between items-center">
          <h3 className="text-xs font-bold uppercase tracking-widest text-red-500">
            Blacklist Registry
          </h3>
          <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
        </div>
        <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-4">
          {!token ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Select a token to view blacklist
            </div>
          ) : !token.complianceAttached ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Compliance module not attached — blacklist unavailable
            </div>
          ) : loading ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Loading...
            </div>
          ) : entries.length === 0 ? (
            <div className="text-center text-slate-500 text-sm py-8">
              No blacklisted addresses for this token
            </div>
          ) : (
            entries.map((entry, i) => (
              <div key={i} className="space-y-2">
                <div className="p-2 bg-red-500/5 border border-red-500/20 font-mono text-[11px] text-red-400">
                  {entry.address}
                </div>
                <div className="flex gap-2">
                  {entry.reason && (
                    <button
                      onClick={() => {
                        setSelectedReason(entry.reason || "No reason provided");
                        setShowReasonModal(true);
                      }}
                      className="flex-1 text-[10px] py-1 bg-[#1f1f23] hover:bg-[#2d2d33] transition-colors uppercase"
                    >
                      Reason
                    </button>
                  )}
                  <button
                    onClick={() => {
                      setAddressToRemove(entry.addressFull);
                      setShowRemoveModal(true);
                    }}
                    className="flex-1 text-[10px] py-1 bg-red-500/20 text-red-400 hover:bg-red-500 hover:text-white transition-colors uppercase"
                  >
                    Remove
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
        <div className="p-3 border-t border-white/10">
          <button
            onClick={() => setShowAddModal(true)}
            disabled={!token || !token.complianceAttached}
            className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-red-500 hover:text-white transition-colors disabled:opacity-50"
          >
            address | blacklist
          </button>
        </div>
      </section>

      {showAddModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px]">
            <h3 className="text-sm font-bold uppercase tracking-widest text-red-500 mb-4">
              Add to Blacklist
            </h3>
            <input
              type="text"
              value={addressToBlacklist}
              onChange={(e) => setAddressToBlacklist(e.target.value)}
              placeholder="Enter wallet address"
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-3 focus:border-red-500 outline-none"
            />
            <input
              type="text"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Reason (optional)"
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-4 focus:border-red-500 outline-none"
            />
            <div className="flex gap-3">
              <button
                onClick={() => {
                  setShowAddModal(false);
                  setAddressToBlacklist("");
                  setReason("");
                }}
                className="flex-1 py-2 bg-white/5 text-xs uppercase font-bold hover:bg-white/10 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleAddBlacklist}
                disabled={adding || !addressToBlacklist}
                className="flex-1 py-2 bg-red-500 text-white text-xs uppercase font-bold hover:bg-red-600 transition-colors disabled:opacity-50"
              >
                {adding ? "Adding..." : "Blacklist"}
              </button>
            </div>
          </div>
        </div>
      )}

      {showAddSuccess && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px] text-center">
            <div className="w-12 h-12 bg-red-500/20 rounded-full flex items-center justify-center mx-auto mb-4">
              <svg
                className="w-6 h-6 text-red-500"
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
              Address Blacklisted!
            </h3>
            <p className="text-xs text-slate-400 mb-6">
              The address has been successfully added to the blacklist.
            </p>
            <button
              onClick={() => setShowAddSuccess(false)}
              className="w-full py-2 bg-red-500 text-white text-xs uppercase font-bold hover:bg-red-600 transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}

      {showRemoveModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px]">
            <h3 className="text-sm font-bold uppercase tracking-widest text-[#25d1f4] mb-4">
              Remove from Blacklist
            </h3>
            <input
              type="text"
              value={addressToRemove}
              onChange={(e) => setAddressToRemove(e.target.value)}
              placeholder="Enter wallet address"
              className="w-full px-3 py-2 bg-[#141417] border border-white/10 rounded text-sm mb-4 focus:border-[#25d1f4] outline-none"
            />
            <div className="flex gap-3">
              <button
                onClick={() => {
                  setShowRemoveModal(false);
                  setAddressToRemove("");
                }}
                className="flex-1 py-2 bg-white/5 text-xs uppercase font-bold hover:bg-white/10 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleRemoveBlacklist}
                disabled={removing || !addressToRemove}
                className="flex-1 py-2 bg-[#25d1f4] text-black text-xs uppercase font-bold hover:bg-[#25d1f4]/90 transition-colors disabled:opacity-50"
              >
                {removing ? "Removing..." : "Remove"}
              </button>
            </div>
          </div>
        </div>
      )}

      {showRemoveSuccess && (
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
              Removed from Blacklist!
            </h3>
            <p className="text-xs text-slate-400 mb-6">
              The address has been successfully removed from the blacklist.
            </p>
            <button
              onClick={() => setShowRemoveSuccess(false)}
              className="w-full py-2 bg-[#25d1f4] text-black text-xs uppercase font-bold hover:bg-[#25d1f4]/90 transition-colors"
            >
              Done
            </button>
          </div>
        </div>
      )}

      {showReasonModal && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-[#1a1a1f] p-6 rounded-lg border border-white/10 w-[400px]">
            <h3 className="text-sm font-bold uppercase tracking-widest text-red-500 mb-4">
              Blacklist Reason
            </h3>
            <div className="bg-[#141417] border border-white/10 rounded px-4 py-3 font-mono text-sm text-slate-300 min-h-[60px]">
              {selectedReason}
            </div>
            <button
              onClick={() => setShowReasonModal(false)}
              className="mt-4 w-full py-2 bg-white/5 text-xs uppercase font-bold hover:bg-white/10 transition-colors"
            >
              Close
            </button>
          </div>
        </div>
      )}
    </>
  );
}
