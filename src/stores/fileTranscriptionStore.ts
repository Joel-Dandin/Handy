import { create } from "zustand";
import { transcribeWavFile } from "@/lib/commands/fileTranscription";

interface FileTranscriptionStore {
  path: string | null;
  isTranscribing: boolean;
  transcript: string | null;
  error: string | null;
  setPath: (path: string) => void;
  transcribe: () => Promise<void>;
}

// Lives outside the panel component so a transcription keeps running, and its
// result is kept, while the user visits other sidebar sections.
export const useFileTranscriptionStore = create<FileTranscriptionStore>(
  (set, get) => ({
    path: null,
    isTranscribing: false,
    transcript: null,
    error: null,

    setPath: (path) => set({ path, transcript: null, error: null }),

    transcribe: async () => {
      const { path, isTranscribing } = get();
      if (!path || isTranscribing) return;

      set({ isTranscribing: true, transcript: null, error: null });
      try {
        const result = await transcribeWavFile(path);
        if (result.status === "ok") {
          set({ transcript: result.data });
        } else {
          set({ error: result.error });
        }
      } catch (err) {
        set({ error: String(err) });
      } finally {
        set({ isTranscribing: false });
      }
    },
  }),
);
