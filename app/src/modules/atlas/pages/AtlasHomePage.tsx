import { useAppHeader } from "@/components/AppShell";

export default function AtlasHomePage() {
  useAppHeader({});

  return (
    <main className="flex flex-1 items-center justify-center p-6">
      <h2 className="text-2xl font-semibold tracking-tight">Previa Atlas</h2>
    </main>
  );
}
