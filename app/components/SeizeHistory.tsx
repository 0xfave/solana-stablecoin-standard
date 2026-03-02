const seized = [
  { address: "0xdead...beef", amount: "1,500 SSS", txn: "0x921...aa2" },
  { address: "0xcafe...babe", amount: "80.00 SSS", txn: "0x551...bb9" },
];

export default function SeizeHistory() {
  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">Seize History</h3>
        <span className="text-[10px] font-mono opacity-50">#SZE-04</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
        {seized.map((item, i) => (
          <div key={i} className="bg-[#141417] p-3 rounded border border-white/5">
            <div className="flex justify-between items-start mb-2">
              <span className="text-[10px] font-mono text-slate-400">{item.address}</span>
              <span className="text-xs font-mono text-[#25d1f4]">{item.amount}</span>
            </div>
            <div className="text-[10px] opacity-30 font-mono">TXN: {item.txn}</div>
          </div>
        ))}
      </div>
      <div className="p-3 border-t border-white/10">
        <button className="w-full py-2 bg-white/5 text-[10px] uppercase font-bold hover:bg-[#25d1f4] hover:text-black transition-colors">
          address | seize
        </button>
      </div>
    </section>
  );
}
