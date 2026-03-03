import { SssToken } from "@/lib/useSolana";

interface RoleManagementProps {
  token: SssToken | null;
}

export default function RoleManagement({ token }: RoleManagementProps) {
  const roles = token ? [
    { address: token.authority, role: "Admin" },
    { address: `${token.mint.slice(0, 6)}...${token.mint.slice(-4)}`, role: "Minter" },
    { address: `${token.mint.slice(0, 6)}...${token.mint.slice(-4)}`, role: "Freezer" },
  ] : [];

  const tokenLabel = token ? (token.name || token.symbol || `${token.mint.slice(0, 8)}...${token.mint.slice(-4)}`) : "Select a token";

  return (
    <section className="osint-card flex flex-col h-[400px]">
      <div className="p-4 border-b border-white/10 flex justify-between items-center">
        <h3 className="text-xs font-bold uppercase tracking-widest text-[#25d1f4]">Role Management</h3>
        <span className="text-[10px] font-mono opacity-50">{tokenLabel}</span>
      </div>
      <div className="flex-1 overflow-y-auto p-4 custom-scrollbar space-y-4">
        {!token ? (
          <div className="text-center text-slate-500 text-sm py-8">
            Select a token to view roles
          </div>
        ) : roles.length === 0 ? (
          <div className="text-center text-slate-500 text-sm py-8">
            No roles for this token
          </div>
        ) : (
          roles.map((item, i) => (
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
          ))
        )}
      </div>
      <div className="p-3 border-t border-white/10">
        <button className="w-full py-2 bg-[#25d1f4] text-black text-[10px] uppercase font-bold hover:bg-white transition-colors">
          Assign New Role
        </button>
      </div>
    </section>
  );
}
