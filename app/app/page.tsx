"use client";

import Header from "@/components/Header";
import TokenTable from "@/components/TokenTable";
import MintHistory from "@/components/MintHistory";
import FreezeHistory from "@/components/FreezeHistory";
import BlacklistPanel from "@/components/BlacklistPanel";
import SeizeHistory from "@/components/SeizeHistory";
import RoleManagement from "@/components/RoleManagement";
import BurnHistory from "@/components/BurnHistory";
import Footer from "@/components/Footer";
import { useSolana } from "@/lib/useSolana";

export default function Home() {
  const { selectedToken } = useSolana();

  return (
    <>
      <Header />
      <main className="max-w-7xl mx-auto space-y-6 p-4 lg:p-8">
        <TokenTable />
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          <MintHistory token={selectedToken} />
          <FreezeHistory token={selectedToken} />
          <BlacklistPanel token={selectedToken} />
          <SeizeHistory token={selectedToken} />
          <RoleManagement token={selectedToken} />
          <BurnHistory token={selectedToken} />
        </div>
      </main>
      <Footer />
    </>
  );
}
