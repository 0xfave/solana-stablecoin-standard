const frozenAccounts = [
  "SSS_PROTOCOL_v1",
  "NEO_GENESIS_SSS",
  "SSS_PROTOCOL_v1",
];

export default function FreezeHistory() {
  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">Freeze History</h3>
        <span className="text-[10px] font-mono opacity-50">#FRZ-12</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
        {frozenAccounts.map((token, i) => (
          <div key={i} className="bg-[#141417] p-3 rounded flex justify-between items-center border border-white/5">
            <span className="text-xs font-bold">{token}</span>
            <button className="text-[10px] px-3 py-1 bg-[#25d1f4]/10 border border-[#25d1f4]/40 text-[#25d1f4] hover:bg-[#25d1f4] hover:text-black transition-colors uppercase">
              Thaw
            </button>
          </div>
        ))}
      </div>
      <div className="p-3 border-t border-white/10">
        <button className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-[#25d1f4] hover:text-black transition-colors">
          address | freeze/thaw
        </button>
      </div>
    </section>
  );
}
