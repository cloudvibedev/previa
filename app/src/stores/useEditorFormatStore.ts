import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { FormatType } from "@/lib/pipeline-schema";

interface EditorFormatState {
  format: FormatType;
  setFormat: (format: FormatType) => void;
}

export const useEditorFormatStore = create<EditorFormatState>()(
  persist(
    (set) => ({
      format: "json",
      setFormat: (format) => set({ format }),
    }),
    { name: "editor-format" }
  )
);
