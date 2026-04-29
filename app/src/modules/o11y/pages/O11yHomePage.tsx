import { useAppHeader } from "@/components/AppShell";

export default function O11yHomePage() {
  useAppHeader({});

  return (
    <main className="flex flex-1 items-center justify-center p-6">
      <h2 className="text-2xl font-semibold tracking-tight">Previa Watch</h2>
    </main>
  );
}
