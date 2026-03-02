import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "SSS Token Dashboard - WorldMonitor OSINT",
  description: "Solana Stablecoin Standard Admin Dashboard",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" className="dark">
      <body className="min-h-screen">
        {children}
      </body>
    </html>
  );
}
