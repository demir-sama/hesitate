import type { Metadata } from "next";
import { Bricolage_Grotesque, IBM_Plex_Mono, Instrument_Serif } from "next/font/google";
import "./globals.css";
const sans = Bricolage_Grotesque({ subsets: ["latin"], variable: "--f-sans" });
const mono = IBM_Plex_Mono({ subsets: ["latin"], weight: ["400", "500"], variable: "--f-mono" });
const serif = Instrument_Serif({ subsets: ["latin"], weight: "400", style: ["normal", "italic"], variable: "--f-serif" });
export const metadata: Metadata = {
  title: "Hesitate — a wallet that pauses before your AI agent does something stupid",
  description: "AI agents propose NEAR transactions; an on-chain policy vault plus an AI sentinel holds drains, look-alikes, prompt injections and anomalies for the owner.",
};
export default function RootLayout({ children }: { children: React.ReactNode }) {
  return <html lang="en" className={`${sans.variable} ${mono.variable} ${serif.variable}`}><body>{children}</body></html>;
}
