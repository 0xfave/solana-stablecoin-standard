const burnHistory = [
  { token: "NEO_GENESIS_SSS", amount: "-100,000.00", txn: "0xcc4...77d", time: "2d ago" },
  { token: "SSS_PROTOCOL_v1", amount: "-500.00", txn: "0xaa2...11b", time: "3d ago" },
];

export default function BurnHistory() {
  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-orange-500">Burn History</h3>
        <span className="text-[10px] font-mono opacity-50">#BRN-42</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
        {burnHistory.map((item, i) => (
          <div key={i} className={`bg-[#141417] p-3 rounded border-l-2 ${i === 0 ? 'border-orange-500' : 'border-orange-500/40'}`}>
            <div className="flex justify-between items-start mb-2">
              <span className="text-xs font-bold text-orange-400">{item.token}</span>
              <span className="text-xs font-mono text-orange-400">{item.amount}</span>
            </div>
            <div className="flex justify-between items-center text-[10px] opacity-50 font-mono">
              <span>TXN: {item.txn}</span>
              <span>{item.time}</span>
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
