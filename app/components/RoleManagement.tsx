const roles = [
  { address: "0x71C7656EC7ab88b098defB751B7401B5f6d8976F", role: "Admin" },
  { address: "0x250d1ba2a99d28e239e262a5436c637492c64b61", role: "Minter" },
  { address: "0x4b71...a92b", role: "Freezer" },
  { address: "0x1111...2222", role: "Viewer" },
];

export default function RoleManagement() {
  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">Role Management</h3>
        <span className="text-[10px] font-mono opacity-50">SYS_ACL</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-4">
        {roles.map((item, i) => (
          <div key={i} className="flex items-center justify-between gap-4">
            <span className="text-[11px] font-mono truncate opacity-60">{item.address}</span>
            <span className={`text-[10px] px-3 py-1 rounded-sm uppercase font-bold ${
              item.role === "Admin" 
                ? "bg-[#25d1f4]/10 border border-[#25d1f4] text-[#25d1f4]" 
                : "bg-white/10 border border-white/20 text-white"
            }`}>
              {item.role}
            </span>
          </div>
        ))}
      </div>
      <div className="p-3 border-t border-white/10">
        <button className="w-full py-2 bg-[#25d1f4] text-black text-[10px] uppercase font-bold hover:bg-white transition-colors">
          Assign New Role
        </button>
      </div>
    </section>
  );
}
