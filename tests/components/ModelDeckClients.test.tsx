import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppSwitcher } from "@/components/AppSwitcher";
import { AppVisibilitySettings } from "@/components/settings/AppVisibilitySettings";
import { resolveClientVisibility } from "@/config/modeldeckClients";

const requestedClients = [
  "Codex CLI",
  "Claude Code",
  "Claude Desktop",
  "Antigravity CLI",
  "Cursor CLI",
  "Cursor GUI",
  "Grok",
  "Muse Code",
  "OpenCode",
  "Hermes",
];

describe("ModelDeck client scope", () => {
  it("offers exactly the requested clients even when imported settings enable other tools", () => {
    const onSwitch = vi.fn();
    render(
      <AppSwitcher
        activeApp="codex"
        onSwitch={onSwitch}
        visibleApps={{
          ...resolveClientVisibility(),
          gemini: true,
          openclaw: true,
          pi: true,
          mcode: true,
        }}
      />,
    );
    for (const name of requestedClients)
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(10);
    fireEvent.click(screen.getByRole("button", { name: "Cursor CLI" }));
    expect(onSwitch).toHaveBeenCalledWith("cursor-agent");
  });

  it("counts only supported clients when preventing the last visible client from being hidden", () => {
    const visibility = resolveClientVisibility();
    for (const key of Object.keys(visibility))
      visibility[key as keyof typeof visibility] = false;
    Object.assign(visibility, { codex: true, openclaw: true, mcode: true });
    render(
      <AppVisibilitySettings
        settings={{
          visibleApps: visibility,
          showInTray: true,
          minimizeToTrayOnClose: true,
          language: "en",
        }}
        onChange={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Codex CLI" })).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: /OpenClaw|MiniMax|Gemini CLI|Pi$/ }),
    ).not.toBeInTheDocument();
  });
});
