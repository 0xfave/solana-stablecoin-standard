"use client";

import { useContext, useState, useEffect } from "react";
import { WalletContext } from "../app/providers";

export default function Header() {
  const { connected, publicKey, connectWallet, disconnectWallet } = useContext(WalletContext);
  const [mounted, setMounted] = useState(false);
  const [showOptions, setShowOptions] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const handleDisconnect = async () => {
    await disconnectWallet();
    setShowOptions(false);
  };

  if (!mounted) {
    return (
      <header className="max-w-7xl mx-auto mb-8 flex justify-between items-center p-4 lg:p-8">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-[#25d1f4] flex items-center justify-center rotate-45">
            <span className="text-black font-bold -rotate-45">S</span>
          </div>
          <h1 className="text-2xl font-bold tracking-tighter text-[#25d1f4] neon-text">
            SSS TOKEN <span className="text-xs font-mono font-normal opacity-50 ml-2">V0.1.0-STABLE</span>
          </h1>
        </div>
      </header>
    );
  }

  const displayAddress = publicKey ? `${publicKey.slice(0, 6)}...${publicKey.slice(-4)}` : null;

  return (
    <header className="max-w-7xl mx-auto mb-8 flex justify-between items-center p-4 lg:p-8">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 bg-[#25d1f4] flex items-center justify-center rotate-45">
          <span className="text-black font-bold -rotate-45">S</span>
        </div>
        <h1 className="text-2xl font-bold tracking-tighter text-[#25d1f4] neon-text">
          SSS TOKEN <span className="text-xs font-mono font-normal opacity-50 ml-2">V0.1.0-STABLE</span>
        </h1>
      </div>

      <div className="relative">
        {connected && displayAddress ? (
          <button
            onClick={() => setShowOptions(!showOptions)}
            className="px-6 py-2 bg-green-500/20 border border-green-500 text-green-400 hover:bg-green-500 hover:text-black transition-all duration-300 font-bold text-xs uppercase tracking-widest neon-border"
          >
            {displayAddress}
          </button>
        ) : (
          <button
            onClick={connectWallet}
            className="px-6 py-2 bg-[#25d1f4]/10 border border-[#25d1f4] text-[#25d1f4] hover:bg-[#25d1f4] hover:text-black transition-all duration-300 font-bold text-xs uppercase tracking-widest neon-border"
          >
            Connect Wallet
          </button>
        )}

        {showOptions && (
          <div className="absolute right-0 mt-2 w-48 bg-[#141417] border border-white/10 rounded-md overflow-hidden z-50">
            <button
              onClick={handleDisconnect}
              className="w-full px-4 py-2 text-left text-xs text-red-400 hover:bg-red-500/20 transition-colors"
            >
              Disconnect
            </button>
          </div>
        )}
      </div>
    </header>
  );
}
