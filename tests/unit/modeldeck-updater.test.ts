import { describe, expect, it, vi } from "vitest";

const { check } = vi.hoisted(() => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check }));

import { checkForUpdate } from "@/lib/updater";

describe("ModelDeck local release updates", () => {
  it("does not contact an inherited updater even if it advertises a newer version", async () => {
    check.mockResolvedValue({ version: "99.0.0" });

    expect(await checkForUpdate()).toEqual({ status: "disabled" });
    expect(check).not.toHaveBeenCalled();
  });
});
