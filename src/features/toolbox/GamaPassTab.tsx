import { useEffect, useState } from "react";
import { useTranslation } from "../../lib/i18n";
import { commands } from "../../lib/tauri";
import { errorMessage } from "../../lib/errors";
import { Section, Row, RowButton, Segmented } from "./ToolboxUi";
import type { PasskeySettingsDto } from "../../lib/types";

/**
 * GamaPass passkey source. Choosing a manager that is not installed yet
 * downloads it first; the choice takes effect on the next GamaPass window.
 */
export function GamaPassTab() {
  const { t } = useTranslation();
  const [passkey, setPasskey] = useState<PasskeySettingsDto | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    commands
      .passkeySettingsGet()
      .then(setPasskey)
      .catch(() => {});
  }, []);

  async function run(key: string, action: () => Promise<PasskeySettingsDto>) {
    setBusy(key);
    setError("");
    try {
      setPasskey(await action());
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }

  function chooseSource(source: string) {
    const manager = passkey?.managers.find((m) => m.key === source);
    if (manager && !manager.installed) {
      void run(`install:${source}`, async () => {
        await commands.passkeyManagerInstall(source);
        return commands.passkeySourceSet(source);
      });
      return;
    }
    void run(`source:${source}`, () => commands.passkeySourceSet(source));
  }

  const shownSource = busy?.startsWith("install:")
    ? busy.slice("install:".length)
    : (passkey?.source ?? "windows");

  return (
    <div className="flex flex-col gap-3">
      <p className="px-1 text-[11px] leading-relaxed text-text-dim">
        {t("settings.passkey_source_desc")}
      </p>
      <Section title={t("toolbox.tabs.gamapass")}>
        <Row label={t("settings.passkey_source")} hint={t("settings.passkey_source_others")}>
          <Segmented
            options={[
              { value: "windows", label: t("settings.passkey_source_windows") },
              ...(passkey?.managers ?? []).map((m) => ({ value: m.key, label: m.name })),
            ]}
            value={shownSource}
            onChange={chooseSource}
          />
        </Row>
        {passkey?.managers.map((m) => (
          <Row
            key={m.key}
            label={m.name}
            hint={
              m.installed
                ? `${m.installed.version} · ${t("settings.passkey_installed")}`
                : busy === `install:${m.key}`
                  ? t("settings.passkey_installing")
                  : t("settings.passkey_not_installed")
            }
          >
            {m.installed && (
              <>
                <RowButton
                  onClick={() =>
                    commands.passkeyManagerOpen(m.key).catch((e) => setError(errorMessage(e)))
                  }
                >
                  {t("settings.passkey_open")}
                </RowButton>
                <RowButton
                  danger
                  onClick={() => run(`remove:${m.key}`, () => commands.passkeyManagerRemove(m.key))}
                >
                  {t("settings.passkey_remove")}
                </RowButton>
              </>
            )}
          </Row>
        ))}
        {passkey && passkey.source !== "windows" && (
          <div className="px-3.5 pb-3 text-[10.5px] leading-snug text-text-faint">
            {t("settings.passkey_manager_hint")}
          </div>
        )}
        {error && <div className="px-3.5 pb-3 text-[11px] text-[var(--danger)]">{error}</div>}
      </Section>
    </div>
  );
}
