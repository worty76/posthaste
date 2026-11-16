import { useEffect, useState, type ReactNode } from "react";
import { Button } from "../components/Button";
import { TextField } from "../components/Controls";
import { api, toAppError } from "../lib/ipc";
import type { AiOverview, AiProviderInfo, Settings } from "../lib/types";

interface Props {
  settings: Settings;
  set: (patch: Partial<Settings>) => Promise<unknown>;
  notify: (text: string, kind?: "info" | "error") => void;
}

type Test = { provider: string } & (
  { kind: "running" } | { kind: "ok"; text: string } | { kind: "failed"; text: string }
);

/**
 * AI polish: pick one provider, then set up only that one. Keys go to the
 * system keychain through their own commands; the page never holds a saved
 * key and never puts one in the settings.
 */
export function AiSection({ settings, set, notify }: Props) {
  const [overview, setOverview] = useState<AiOverview | null>(null);
  const [failed, setFailed] = useState(false);
  const [test, setTest] = useState<Test | null>(null);

  // Reloaded whenever the provider changes, so a key removed elsewhere
  // ("Clear all data") never still shows as saved.
  useEffect(() => {
    let current = true;
    api.aiProviders().then(
      (o) => {
        if (!current) return;
        setOverview(o);
        setFailed(false);
      },
      (e) => {
        if (!current) return;
        setFailed(true);
        notify(toAppError(e).message, "error");
      },
    );
    return () => {
      current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings.ai_provider]);

  const provider = overview?.providers.find((p) => p.id === settings.ai_provider) ?? null;
  const unknown = settings.ai_provider !== "none" && overview !== null && !provider;
  // A result belongs to the provider it ran against; a late one is dropped.
  const shown = test?.provider === settings.ai_provider ? test : null;

  const runTest = async (p: AiProviderInfo) => {
    setTest({ provider: p.id, kind: "running" });
    try {
      setTest({ provider: p.id, kind: "ok", text: await api.aiTest(p.id) });
    } catch (e) {
      setTest({ provider: p.id, kind: "failed", text: toAppError(e).message });
    }
  };

  return (
    <section className="settings__section" aria-labelledby="s-ai">
      <h2 className="settings__heading" id="s-ai">
        AI polish
      </h2>
      <Row
        label="Polish with"
        help="Off by default. Tidies a note's spelling and grammar when you ask, never on its own."
      >
        <select
          className="field select"
          aria-label="Polish with"
          value={settings.ai_provider}
          disabled={!overview && !failed}
          onChange={(e) => void set({ ai_provider: e.target.value })}
        >
          <option value="none">Off</option>
          {(unknown || (failed && settings.ai_provider !== "none")) && (
            <option value={settings.ai_provider} disabled>
              {settings.ai_provider} (not available)
            </option>
          )}
          {overview?.providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </Row>

      {provider && overview && (
        <>
          <p className="ai__about">
            {provider.key === "none"
              ? provider.key_help
              : `${provider.key_help} Polishing sends the note text to ${provider.name}.`}
            {provider.key_url && (
              <>
                {" "}
                <button
                  type="button"
                  className="link"
                  onClick={() =>
                    api.aiKeyPage(provider.id).catch((e) => notify(toAppError(e).message, "error"))
                  }
                >
                  Get a key
                </button>
              </>
            )}
          </p>

          {provider.key !== "none" && (
            <KeyRow
              key={provider.id}
              provider={provider}
              backend={overview.backend}
              onChange={setOverview}
              notify={notify}
            />
          )}

          {provider.base_url_editable && (
            <Row label="Address" help="Where the server listens, ending in /v1 for most.">
              <TextField
                aria-label={`${provider.name} address`}
                value={settings.ai_base_urls[provider.id] ?? ""}
                placeholder={provider.base_url || "Server address"}
                onChange={(e) =>
                  void set({
                    ai_base_urls: { ...settings.ai_base_urls, [provider.id]: e.target.value },
                  })
                }
                spellCheck={false}
                autoComplete="off"
              />
            </Row>
          )}

          <Row
            label="Model"
            help={
              provider.default_model
                ? "Leave empty for the small, fast default."
                : "The model name the server expects."
            }
          >
            <TextField
              aria-label={`${provider.name} model`}
              value={settings.ai_models[provider.id] ?? ""}
              placeholder={provider.default_model || "Model name"}
              onChange={(e) =>
                void set({ ai_models: { ...settings.ai_models, [provider.id]: e.target.value } })
              }
              spellCheck={false}
              autoComplete="off"
            />
          </Row>

          <Row label="Check the setup" help="Polishes a short sample note.">
            <Button
              variant="quiet"
              onClick={() => void runTest(provider)}
              disabled={shown?.kind === "running"}
            >
              {shown?.kind === "running" ? "Testing…" : "Test"}
            </Button>
          </Row>
          {shown?.kind === "ok" && (
            <p className="status ai__result selectable" data-state="ok" role="status">
              It works. It returned: {shown.text}
            </p>
          )}
          {shown?.kind === "failed" && (
            <p className="status ai__result selectable" data-state="error" role="alert">
              {shown.text}
            </p>
          )}
        </>
      )}
    </section>
  );
}

function KeyRow({
  provider,
  backend,
  onChange,
  notify,
}: {
  provider: AiProviderInfo;
  backend: AiOverview["backend"];
  onChange: (o: AiOverview) => void;
  notify: Props["notify"];
}) {
  const [draft, setDraft] = useState("");
  const [replacing, setReplacing] = useState(false);
  const [busy, setBusy] = useState(false);

  const run = async (task: () => Promise<AiOverview>) => {
    setBusy(true);
    try {
      onChange(await task());
      setDraft("");
      setReplacing(false);
    } catch (e) {
      notify(toAppError(e).message, "error");
    } finally {
      setBusy(false);
    }
  };

  const save = () => {
    const key = draft.trim();
    if (key) void run(() => api.aiKeySet(provider.id, key));
  };

  const label = (
    <>API key {provider.key === "optional" && <span className="row__optional">(optional)</span>}</>
  );

  if (provider.has_key && !replacing) {
    return (
      <Row
        label={label}
        help={
          backend === "keychain"
            ? "Saved in your system keychain."
            : "Saved in Posthaste's database: there is no keychain on this system."
        }
      >
        <Button variant="quiet" onClick={() => setReplacing(true)} disabled={busy}>
          Replace
        </Button>
        <Button
          variant="quiet"
          onClick={() => void run(() => api.aiKeyClear(provider.id))}
          disabled={busy}
        >
          Remove
        </Button>
      </Row>
    );
  }

  return (
    <Row label={label}>
      <TextField
        type="password"
        aria-label={`${provider.name} API key`}
        value={draft}
        placeholder="Paste your key"
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) {
            e.preventDefault();
            save();
          }
        }}
        spellCheck={false}
        autoComplete="off"
      />
      <Button onClick={save} disabled={busy || !draft.trim()}>
        Save key
      </Button>
      {replacing && (
        <Button variant="quiet" onClick={() => setReplacing(false)} disabled={busy}>
          Cancel
        </Button>
      )}
    </Row>
  );
}

function Row({
  label,
  help,
  children,
}: {
  label: ReactNode;
  help?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="row">
      <div className="row__text">
        <div className="row__label">{label}</div>
        {help && <div className="row__help">{help}</div>}
      </div>
      <div className="row__control">{children}</div>
    </div>
  );
}
