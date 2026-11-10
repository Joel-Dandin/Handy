import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { Check, Copy, FileAudio } from "lucide-react";
import { toast } from "sonner";
import { useFileTranscriptionStore } from "@/stores/fileTranscriptionStore";
import { Alert } from "../../ui/Alert";
import { Button } from "../../ui/Button";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { Textarea } from "../../ui/Textarea";
import { copyToClipboard } from "../history/clipboard";

export const FileTranscriptionSettings: React.FC = () => {
  const { t } = useTranslation();
  const { path, isTranscribing, transcript, error, setPath, transcribe } =
    useFileTranscriptionStore();
  const [copied, setCopied] = useState(false);

  const chooseFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "WAV", extensions: ["wav"] }],
    });
    if (typeof selected === "string") {
      setPath(selected);
    }
  };

  const copyTranscript = async () => {
    if (!transcript) return;
    if (await copyToClipboard(transcript)) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } else {
      toast.error(t("settings.fileTranscription.copyError"));
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup
        title={t("settings.fileTranscription.title")}
        description={t("settings.fileTranscription.description")}
      >
        <div className="p-4 space-y-3">
          <p className="text-xs text-mid-gray">
            {t("settings.fileTranscription.formatHint")}
          </p>
          <div className="flex items-center gap-2">
            <div className="flex-1 min-w-0 px-2 py-2 bg-mid-gray/10 border border-mid-gray/80 rounded-lg text-xs font-mono break-all select-text cursor-text">
              {path ?? t("settings.fileTranscription.noFile")}
            </div>
            <Button
              variant="secondary"
              size="sm"
              onClick={chooseFile}
              disabled={isTranscribing}
              className="flex items-center gap-2 px-3 py-2"
            >
              <FileAudio className="w-4 h-4" />
              <span>{t("settings.fileTranscription.chooseFile")}</span>
            </Button>
          </div>
          <Button onClick={transcribe} disabled={!path || isTranscribing}>
            {isTranscribing
              ? t("settings.fileTranscription.transcribing")
              : t("settings.fileTranscription.transcribe")}
          </Button>
        </div>
      </SettingsGroup>

      {error && (
        <Alert variant="error">
          {t("settings.fileTranscription.error", { error })}
        </Alert>
      )}

      {transcript !== null && (
        <SettingsGroup title={t("settings.fileTranscription.resultTitle")}>
          <div className="p-4 space-y-3">
            {transcript ? (
              <>
                <Textarea
                  readOnly
                  value={transcript}
                  className="w-full select-text cursor-text"
                />
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={copyTranscript}
                  className="flex items-center gap-2"
                >
                  {copied ? (
                    <Check className="w-4 h-4" />
                  ) : (
                    <Copy className="w-4 h-4" />
                  )}
                  <span>
                    {copied
                      ? t("settings.fileTranscription.copied")
                      : t("settings.fileTranscription.copy")}
                  </span>
                </Button>
              </>
            ) : (
              <p className="text-sm text-mid-gray">
                {t("settings.fileTranscription.empty")}
              </p>
            )}
          </div>
        </SettingsGroup>
      )}
    </div>
  );
};
