export default function Header() {
  return (
    <header className="max-w-7xl mx-auto mb-8 flex justify-between items-center p-4 lg:p-8">
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 bg-[#25d1f4] flex items-center justify-center rotate-45">
          <span className="text-black font-bold -rotate-45">S</span>
        </div>
        <h1 className="text-2xl font-bold tracking-tighter text-[#25d1f4] neon-text">
          SSS TOKEN <span className="text-xs font-mono font-normal opacity-50 ml-2">V2.0.48-STABLE</span>
        </h1>
      </div>
      <button className="px-6 py-2 bg-[#25d1f4]/10 border border-[#25d1f4] text-[#25d1f4] hover:bg-[#25d1f4] hover:text-black transition-all duration-300 font-bold text-xs uppercase tracking-widest neon-border">
        Connect Wallet
      </button>
    </header>
  );
}
