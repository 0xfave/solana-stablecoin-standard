export default function Footer() {
  return (
    <footer className="max-w-7xl mx-auto mt-12 pt-4 pb-8 border-t border-white/5 flex justify-between items-center text-[10px] font-mono text-slate-500 uppercase tracking-widest px-4 lg:px-8">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-green-500"></span>
          Network: Mainnet
        </div>
        <div>Latency: 24ms</div>
      </div>
      <div>© 2024 WORLDMONITOR.IO / SSS PROTOCOL</div>
    </footer>
  );
}
