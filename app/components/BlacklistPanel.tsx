const blacklistedAddresses = [
  { address: "0x1234...abcd", reason: "Suspected fraud" },
  { address: "0x9988...7766", reason: "Sanctions violation" },
];

export default function BlacklistPanel() {
  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-red-500">Blacklist Registry</h3>
        <span className="text-[10px] font-mono opacity-50">#BLK-99</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-4">
        {blacklistedAddresses.map((entry, i) => (
          <div key={i} className="space-y-2">
            <div className="p-2 bg-red-500/5 border border-red-500/20 font-mono text-[11px] text-red-400">
              {entry.address}
            </div>
            <div className="flex gap-2">
              <button className="flex-1 text-[10px] py-1 bg-[#1f1f23] hover:bg-[#2d2d33] transition-colors uppercase">
                Reason
              </button>
              <button className="flex-1 text-[10px] py-1 bg-red-500/20 text-red-400 hover:bg-red-500 hover:text-white transition-colors uppercase">
                Remove
              </button>
            </div>
          </div>
        ))}
      </div>
      <div className="p-3 border-t border-white/10">
        <button className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-[#25d1f4] hover:text-black transition-colors">
          address | blacklist
        </button>
      </div>
    </section>
  );
}
