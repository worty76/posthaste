import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../lib/types";
import { aiOverview, api, defaultSettings, resetIpc } from "../test/mockIpc";
import { AiSection } from "./AiSection";

vi.mock("../lib/ipc", async () => (await import("../test/mockIpc")).ipcMock);

beforeEach(() => resetIpc());

function setup(patch: Partial<Settings> = {}) {
  const settings: Settings = { ...defaultSettings, ...patch };
  const set = vi.fn(async (patch: Partial<Settings>) => void patch);
  const notify = vi.fn();
  const view = render(<AiSection settings={settings} set={set} notify={notify} />);
  const rerender = (next: Partial<Settings>) =>
    view.rerender(<AiSection settings={{ ...settings, ...next }} set={set} notify={notify} />);
  return { set, notify, rerender, user: userEvent.setup() };
}

const picker = () => screen.findByLabelText("Polish with");

describe("AI section", () => {
  it("is off by default and lists every provider", async () => {
    setup();
    const select = (await picker()) as HTMLSelectElement;
    expect(select.value).toBe("none");
    expect(Array.from(select.options).map((o) => o.textContent)).toEqual([
      "Off",
      "Claude",
      "OpenAI",
      "GitHub Models",
      "Ollama",
      "OpenAI-compatible",
    ]);
    expect(screen.queryByLabelText(/API key/)).toBeNull();
  });

  it("choosing a provider saves it", async () => {
    const { set, user } = setup();
    await user.selectOptions(await picker(), "github");
    expect(set).toHaveBeenCalledWith({ ai_provider: "github" });
  });

  it("saves a pasted key to the keychain, never into the settings", async () => {
    api.aiKeySet.mockResolvedValue(aiOverview(["openai"]));
    const { set, user } = setup({ ai_provider: "openai" });
    const field = await screen.findByLabelText("OpenAI API key");
    expect(field).toHaveAttribute("type", "password");
    await user.type(field, "  sk-test  ");
    await user.click(screen.getByRole("button", { name: "Save key" }));
    expect(api.aiKeySet).toHaveBeenCalledWith("openai", "sk-test");
    expect(await screen.findByText("Saved in your system keychain.")).toBeInTheDocument();
    expect(screen.queryByLabelText("OpenAI API key")).toBeNull();
    expect(JSON.stringify(set.mock.calls)).not.toContain("sk-test");
  });

  it("Enter in the key field saves it too", async () => {
    api.aiKeySet.mockResolvedValue(aiOverview(["openai"]));
    const { user } = setup({ ai_provider: "openai" });
    await user.type(await screen.findByLabelText("OpenAI API key"), "sk-1{Enter}");
    expect(api.aiKeySet).toHaveBeenCalledWith("openai", "sk-1");
  });

  it("a saved key can be replaced or removed, and is never shown", async () => {
    api.aiProviders.mockResolvedValue(aiOverview(["anthropic"]));
    api.aiKeyClear.mockResolvedValue(aiOverview());
    const { user } = setup({ ai_provider: "anthropic" });
    expect(await screen.findByText("Saved in your system keychain.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Replace" }));
    expect(screen.getByLabelText("Claude API key")).toHaveValue("");
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(api.aiKeyClear).toHaveBeenCalledWith("anthropic");
    expect(await screen.findByLabelText("Claude API key")).toBeInTheDocument();
  });

  it("says when keys are kept in the database because there is no keychain", async () => {
    api.aiProviders.mockResolvedValue(aiOverview(["openai"], "database"));
    setup({ ai_provider: "openai" });
    expect(await screen.findByText(/no keychain on this system/)).toBeInTheDocument();
  });

  it("links to where the provider hands out keys", async () => {
    const { user } = setup({ ai_provider: "github" });
    expect(await screen.findByText(/Models: read permission/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Get a key" }));
    expect(api.aiKeyPage).toHaveBeenCalledWith("github");
  });

  it("a local provider needs no key and says notes stay on this computer", async () => {
    setup({ ai_provider: "ollama" });
    expect(await screen.findByText(/Notes never leave it/)).toBeInTheDocument();
    expect(screen.queryByLabelText(/API key/)).toBeNull();
    expect(screen.getByLabelText("Ollama address")).toHaveAttribute(
      "placeholder",
      "localhost:11434/v1",
    );
  });

  it("a remote provider says the note text is sent to it", async () => {
    setup({ ai_provider: "openai" });
    expect(await screen.findByText(/sends the note text to OpenAI/)).toBeInTheDocument();
  });

  it("keeps a model and address per provider", async () => {
    const { set } = setup({
      ai_provider: "custom",
      ai_models: { openai: "gpt-4.1-nano" },
      ai_base_urls: {},
    });
    const model = await screen.findByLabelText("OpenAI-compatible model");
    expect(model).toHaveAttribute("placeholder", "Model name");
    fireEvent.change(model, { target: { value: "qwen2.5" } });
    expect(set).toHaveBeenLastCalledWith({
      ai_models: { openai: "gpt-4.1-nano", custom: "qwen2.5" },
    });
    fireEvent.change(screen.getByLabelText("OpenAI-compatible address"), {
      target: { value: "http://localhost:1234/v1" },
    });
    expect(set).toHaveBeenLastCalledWith({
      ai_base_urls: { custom: "http://localhost:1234/v1" },
    });
    expect(screen.getByLabelText("OpenAI-compatible API key")).toBeInTheDocument();
    expect(screen.getByText("(optional)")).toBeInTheDocument();
  });

  it("the model field shows the provider's default until one is typed", async () => {
    setup({ ai_provider: "github" });
    expect(await screen.findByLabelText("GitHub Models model")).toHaveAttribute(
      "placeholder",
      "openai/gpt-4.1-mini",
    );
  });

  it("tests the setup and shows what came back", async () => {
    api.aiTest.mockResolvedValue("Remind me to send the slides to Anna.");
    const { user } = setup({ ai_provider: "ollama" });
    await user.click(await screen.findByRole("button", { name: "Test" }));
    expect(api.aiTest).toHaveBeenCalledWith("ollama");
    expect(await screen.findByText(/It works/)).toHaveTextContent(
      "Remind me to send the slides to Anna.",
    );
  });

  it("shows a failed test as an alert with the reason", async () => {
    api.aiTest.mockRejectedValue({
      code: "ai_provider_error",
      message: "OpenAI did not accept the key. Check it in Settings.",
    });
    api.aiProviders.mockResolvedValue(aiOverview(["openai"]));
    const { user } = setup({ ai_provider: "openai" });
    await user.click(await screen.findByRole("button", { name: "Test" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("did not accept the key");
  });

  it("forgets the last test result when switching provider", async () => {
    api.aiTest.mockResolvedValue("Fine.");
    const { user, rerender } = setup({ ai_provider: "ollama" });
    await user.click(await screen.findByRole("button", { name: "Test" }));
    await screen.findByText(/It works/);
    rerender({ ai_provider: "openai" });
    await waitFor(() => expect(screen.queryByText(/It works/)).toBeNull());
  });

  it("shows a key that will not save as an error", async () => {
    api.aiKeySet.mockRejectedValue({
      code: "keychain",
      message: "The system keychain could not be used.",
    });
    const { notify, user } = setup({ ai_provider: "openai" });
    await user.type(await screen.findByLabelText("OpenAI API key"), "sk-1");
    await user.click(screen.getByRole("button", { name: "Save key" }));
    await waitFor(() =>
      expect(notify).toHaveBeenCalledWith("The system keychain could not be used.", "error"),
    );
  });

  it("announces a working test to screen readers", async () => {
    api.aiTest.mockResolvedValue("Fine.");
    const { user } = setup({ ai_provider: "ollama" });
    await user.click(await screen.findByRole("button", { name: "Test" }));
    expect(await screen.findByRole("status")).toHaveTextContent("It works");
  });

  it("ignores a test result that arrives after switching provider", async () => {
    let finish: (v: string) => void = () => {};
    api.aiTest.mockImplementation(() => new Promise<string>((r) => (finish = r)));
    const { user, rerender } = setup({ ai_provider: "ollama" });
    await user.click(await screen.findByRole("button", { name: "Test" }));
    rerender({ ai_provider: "openai" });
    finish("Late answer.");
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByText(/Late answer/)).toBeNull();
  });

  it("reloads which keys are saved when the provider changes", async () => {
    const { rerender } = setup({ ai_provider: "openai" });
    await screen.findByLabelText("OpenAI API key");
    // A key removed elsewhere (Clear all data) must not still show as saved.
    api.aiProviders.mockResolvedValue(aiOverview(["anthropic"]));
    rerender({ ai_provider: "anthropic" });
    expect(await screen.findByText("Saved in your system keychain.")).toBeInTheDocument();
    expect(api.aiProviders).toHaveBeenCalledTimes(2);
  });

  it("does not carry a typed key over to another provider", async () => {
    const { user, rerender } = setup({ ai_provider: "openai" });
    await user.type(await screen.findByLabelText("OpenAI API key"), "sk-typed");
    rerender({ ai_provider: "anthropic" });
    expect(await screen.findByLabelText("Claude API key")).toHaveValue("");
  });

  it("shows a provider this version does not know, and can still be turned off", async () => {
    const { set, user } = setup({ ai_provider: "mistral" });
    const select = (await picker()) as HTMLSelectElement;
    expect(select.value).toBe("mistral");
    expect(screen.getByRole("option", { name: "mistral (not available)" })).toBeDisabled();
    await user.selectOptions(select, "none");
    expect(set).toHaveBeenCalledWith({ ai_provider: "none" });
  });

  it("can still be turned off when the provider list fails to load", async () => {
    api.aiProviders.mockRejectedValue({ code: "keychain", message: "Keychain locked." });
    const { set, notify, user } = setup({ ai_provider: "openai" });
    await waitFor(() => expect(notify).toHaveBeenCalledWith("Keychain locked.", "error"));
    const select = (await picker()) as HTMLSelectElement;
    expect(select).not.toBeDisabled();
    await user.selectOptions(select, "none");
    expect(set).toHaveBeenCalledWith({ ai_provider: "none" });
  });
});
