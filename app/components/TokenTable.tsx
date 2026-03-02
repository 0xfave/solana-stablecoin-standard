const tokens = [
  { name: "NEO_GENESIS_SSS", supply: "1,000,000,000.00", status: "Active" },
  { name: "SSS_PROTOCOL_v1", supply: "500,000.00", status: "Locked" },
];

export default function TokenTable() {
  return (
    <section className="osint-card p-6 rounded-md relative overflow-hidden">
      <div className="absolute top-0 right-0 p-2 opacity-10 font-mono text-xs">SYS_LOG_09X</div>
      <div className="flex justify-between items-end mb-6">
        <div>
          <h2 className="text-sm font-mono text-[#25d1f4] mb-1 uppercase tracking-widest flex items-center gap-2">
            <span className="w-2 h-2 bg-[#25d1f4] animate-pulse"></span>
            My Assets
          </h2>
          <p className="text-2xl font-bold">Token Management Console</p>
        </div>
        <button className="bg-[#25d1f4] text-black px-4 py-2 text-xs font-bold uppercase hover:bg-white transition-colors">
          Create New Token sss-1/sss-2
        </button>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-left border-collapse">
          <thead className="text-[10px] uppercase font-mono text-slate-500 border-b border-white/5">
            <tr>
              <th className="pb-3 px-4">Token Name</th>
              <th className="pb-3 px-4">Total Supply</th>
              <th className="pb-3 px-4 text-center" colSpan={3}>Management Operations</th>
              <th className="pb-3 px-4 text-right">State</th>
            </tr>
          </thead>
          <tbody className="text-sm">
            {tokens.map((token, i) => (
              <tr key={i} className="border-b border-white/5 hover:bg-white/5 group transition-colors">
                <td className="py-4 px-4 font-bold text-[#25d1f4]">{token.name}</td>
                <td className="py-4 px-4 font-mono">{token.supply}</td>
                <td className="py-4 px-1">
                  <button className="w-full text-[10px] border border-white/20 py-1 hover:border-[#25d1f4] transition-colors uppercase">Add Minter</button>
                </td>
                <td className="py-4 px-1">
                  <button className="w-full text-[10px] border border-white/20 py-1 hover:border-[#25d1f4] transition-colors uppercase">Add Freezer</button>
                </td>
                <td className="py-4 px-1">
                  <button className="w-full text-[10px] bg-[#25d1f4]/20 text-[#25d1f4] border border-[#25d1f4]/40 py-1 hover:bg-[#25d1f4] hover:text-black transition-all uppercase">Select Token</button>
                </td>
                <td className="py-4 px-4 text-right">
                  <span className={`text-[10px] px-2 py-0.5 rounded-full uppercase ${
                    token.status === "Active" 
                      ? "bg-green-500/20 text-green-400" 
                      : "bg-yellow-500/20 text-yellow-400"
                  }`}>
                    {token.status}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="mt-4 text-center">
        <p className="text-[10px] font-mono text-slate-500 animate-bounce">↓ Scroll to show other tokens ↓</p>
      </div>
    </section>
  );
}
