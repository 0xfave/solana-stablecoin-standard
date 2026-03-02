const mintHistory = [
  { token: "NEO_GENESIS_SSS", amount: "+15,000.00", txn: "0x8f2...e31", time: "12m ago" },
  { token: "NEO_GENESIS_SSS", amount: "+5,000.00", txn: "0x4a1...99c", time: "1h ago" },
  { token: "NEO_GENESIS_SSS", amount: "+150,000.00", txn: "0x22b...00f", time: "5h ago" },
];

export default function MintHistory() {
  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">Mint History</h3>
        <span className="text-[10px] font-mono opacity-50">#MNT-88</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-3">
        {mintHistory.map((item, i) => (
          <div key={i} className={`bg-[#141417] p-3 rounded border-l-2 ${i === 0 ? 'border-[#25d1f4]' : 'border-[#25d1f4]/40'}`}>
            <div className="flex justify-between items-start mb-2">
              <span className="text-xs font-bold text-[#25d1f4]">{item.token}</span>
              <span className="text-xs font-mono">{item.amount}</span>
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
