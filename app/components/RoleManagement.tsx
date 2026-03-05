"use client";

import { useState } from "react";
import { SssToken } from "@/lib/useSolana";
import { useSolana } from "@/lib/useSolana";

interface RoleManagementProps {
  token: SssToken | null;
}

type RoleType = "Minter" | "Freezer" | "Blacklister";

export default function RoleManagement({ token }: RoleManagementProps) {
  const { addMinter, addFreezer, addBlacklister } = useSolana();
  const [showModal, setShowModal] = useState(false);
  const [selectedRole, setSelectedRole] = useState<RoleType>("Minter");
  const [address, setAddress] = useState("");
  const [assigning, setAssigning] = useState(false);
  const [showSuccess, setShowSuccess] = useState(false);

  const tokenLabel = token
    ? token.name ||
      token.symbol ||
      `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`
    : "Select a token";

  const roles = token
    ? [
        { address: token.authority, role: "Admin" },
        ...(token.minters ?? []).map((m) => ({ address: m, role: "Minter" })),
        ...(token.freezer ? [{ address: token.freezer, role: "Freezer" }] : []),
        ...(token.blacklister
          ? [{ address: token.blacklister, role: "Blacklister" }]
          : []),
      ]
    : [];

  const shortAddr = (addr: string) => `${addr.slice(0, 6)}...${addr.slice(-4)}`;

  const handleAssign = async () => {
    if (!token || !address) return;
    setAssigning(true);
    try {
      if (selectedRole === "Minter") await addMinter(token, address);
      else if (selectedRole === "Freezer") await addFreezer(token, address);
      else if (selectedRole === "Blacklister")
        await addBlacklister(token, address);
      setShowModal(false);
      setAddress("");
      setShowSuccess(true);
    } catch (err) {
      alert(err instanceof Error ? err.message : "Failed to assign role");
    } finally {
      setAssigning(false);
    }
  };

  return (
    <>
      <section className="osint-card flex flex-col h-[400px]">
        <div className="p-4 border-b border-white/10 flex justify-between items-center">
          <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">
            Role Management
          </h3>
          <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
        </div>

        <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-4">
          {!token ? (
            <div className="text-center text-slate-500 text-sm py-8">
              Select a token to view roles
            </div>
          ) : roles.length === 0 ? (
            <div className="text-center text-slate-500 text-sm py-8">
              No roles for this token
            </div>
          ) : (
            roles.map((item, i) => (
              <div key={i} className="flex items-center justify-between gap-4">
                <span className="text-[11px] font-mono truncate opacity-60">
                  {shortAddr(item.address)}
                </span>
                <span
                  className={`text-[10px] px-3 py-1 rounded-sm uppercase font-bold whitespace-nowrap ${
                    item.role === "Admin"
                      ? "bg-[#25d1f4]/10 border border-[#25d1f4] text-[#25d1f4]"
                      : item.role === "Minter"
                      ? "bg-green-500/10 border border-green-500/40 text-green-400"
                      : item.role === "Freezer"
                      ? "bg-yellow-500/10 border border-yellow-500/40 text-yellow-400"
                      : "bg-red-500/10 border border-red-500/40 text-red-400"
                  }`}
                >
                  {item.role}
                </span>
              </div>
            ))
          )}
        </div>

        <div className="p-3 border-t border-white/10">
          <button
            onClick={() => setShowModal(true)}
            disabled={!token}
            className="w-full py-2 bg-[#25d1f4] text-black text-[10px] uppercase font-bold hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Assign New Role
          </button>
        </div>
      </section>

      {/* Assign Role Modal */}
      {showModal && token && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="osint-card p-6 rounded-md max-w-md w-full mx-4">
            <h3 className="text-lg font-bold mb-4 text-[#25d1f4]">
              Assign New Role
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              Assign a role to a wallet address for token:{" "}
              {token.mint.slice(0, 8)}...{token.mint.slice(-4)}
            </p>
            <div className="space-y-4">
              {/* Role selector */}
              <div>
                <label className="block text-xs text-slate-400 mb-2 uppercase">
                  Role
                </label>
                <div className="flex gap-2">
                  {(["Minter", "Freezer", "Blacklister"] as RoleType[]).map(
                    (role) => (
                      <button
                        key={role}
                        onClick={() => setSelectedRole(role)}
                        className={`flex-1 py-2 text-[10px] uppercase font-bold border transition-colors ${
                          selectedRole === role
                            ? role === "Minter"
                              ? "bg-green-500/20 border-green-500 text-green-400"
                              : role === "Freezer"
                              ? "bg-yellow-500/20 border-yellow-500 text-yellow-400"
                              : "bg-red-500/20 border-red-500 text-red-400"
                            : "border-white/20 text-slate-400 hover:border-white/40"
                        }`}
                      >
                        {role}
                      </button>
                    )
                  )}
                </div>
              </div>

              {/* Address input */}
              <div>
                <label className="block text-xs text-slate-400 mb-1 uppercase">
                  Wallet Address
                </label>
                <input
                  type="text"
                  value={address}
                  onChange={(e) => setAddress(e.target.value)}
                  placeholder="Enter wallet address"
                  className="w-full bg-black/30 border border-white/20 rounded px-3 py-2 text-white placeholder-slate-600 focus:border-[#25d1f4] focus:outline-none"
                />
              </div>

              <button
                onClick={handleAssign}
                disabled={assigning || !address}
                className="w-full bg-[#25d1f4] text-black py-3 font-bold uppercase hover:bg-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {assigning ? "Assigning..." : `Assign ${selectedRole}`}
              </button>
              <button
                onClick={() => {
                  setShowModal(false);
                  setAddress("");
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
              Role Assigned!
            </h3>
            <p className="text-sm text-slate-400 mb-6">
              The {selectedRole} role has been successfully assigned.
            </p>
            <button
              onClick={() => setShowSuccess(false)}
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
