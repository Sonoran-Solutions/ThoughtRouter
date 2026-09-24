import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { invoke } from "../test/tauri-mock";
import { CaptureBox } from "./CaptureBox";

const view = (text: string) => ({
  capture: {
    id: "c1",
    text,
    source: "desktop",
    captured_at: "2026-09-24T00:00:00.000Z",
    deleted_at: null,
  },
  state: "saved",
  last_error: null,
  atoms: [],
  pending_suggestions: [],
  interpreted_by: null,
});

describe("CaptureBox", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("disables save for empty or whitespace-only input", async () => {
    render(<CaptureBox />);
    const button = screen.getByRole("button", { name: "Save Thought" });
    expect(button).toBeDisabled();
    await userEvent.type(screen.getByLabelText("What's on your mind?"), "   ");
    expect(button).toBeDisabled();
  });

  it("saves the exact text with Ctrl+Enter, then clears and confirms", async () => {
    invoke.mockImplementation(async (_cmd: string, args: { text: string }) => view(args.text));
    render(<CaptureBox />);
    const box = screen.getByLabelText("What's on your mind?");
    await userEvent.type(box, "  messy thought{Shift>}{Enter}{/Shift}second line ");
    await userEvent.keyboard("{Control>}{Enter}{/Control}");
    expect(invoke).toHaveBeenCalledWith("save_capture", { text: "  messy thought\nsecond line " });
    expect(await screen.findByRole("status")).toHaveTextContent("Saved");
    expect(box).toHaveValue("");
  });

  it("keeps the text when the save fails", async () => {
    invoke.mockRejectedValue("disk full");
    render(<CaptureBox />);
    const box = screen.getByLabelText("What's on your mind?");
    await userEvent.type(box, "don't lose me");
    await userEvent.click(screen.getByRole("button", { name: "Save Thought" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Not saved: disk full");
    expect(box).toHaveValue("don't lose me");
  });
});
