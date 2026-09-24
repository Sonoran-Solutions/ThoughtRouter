import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import type { AtomView } from "../bindings/AtomView";
import { invoke } from "../test/tauri-mock";
import { AtomRow } from "./AtomRow";
import { Snippet } from "./Chips";
import { ProcessingBanner } from "./ProcessingBanner";

const atomView: AtomView = {
  atom: {
    id: "a1",
    capture_id: "c1",
    run_id: "r1",
    text: "Research the cooler protocol",
    atom_type: "research",
    confidence: 0.8,
    quote: null,
    span_start: null,
    span_end: null,
    origin: "ai",
    status: "active",
    created_at: "",
    updated_at: "",
  },
  links: [
    {
      atom_id: "a1",
      project_id: "p1",
      project_name: "Linux Control Center",
      status: "suggested",
      origin: "ai",
      confidence: 0.9,
      reason: "Same device",
    },
  ],
};

describe("AtomRow", () => {
  beforeEach(() => {
    invoke.mockReset().mockResolvedValue(undefined);
  });

  it("corrects the type, confirms a link and rejects the atom", async () => {
    let changed = 0;
    render(
      <ul>
        <AtomRow view={atomView} onChanged={() => changed++} />
      </ul>,
    );
    await userEvent.selectOptions(screen.getByLabelText("Atom type"), "question");
    expect(invoke).toHaveBeenCalledWith("set_atom_type", { atomId: "a1", atomType: "question" });
    await userEvent.click(screen.getByLabelText("Confirm link to Linux Control Center"));
    expect(invoke).toHaveBeenCalledWith("confirm_link", { atomId: "a1", projectId: "p1" });
    await userEvent.click(screen.getByLabelText("Reject atom"));
    expect(invoke).toHaveBeenCalledWith("reject_atom", { atomId: "a1" });
    expect(changed).toBe(3);
  });

  it("edits the atom text inline", async () => {
    render(
      <ul>
        <AtomRow view={atomView} onChanged={() => {}} />
      </ul>,
    );
    await userEvent.click(screen.getByText("Research the cooler protocol"));
    const input = screen.getByLabelText("Atom text");
    await userEvent.clear(input);
    await userEvent.type(input, "Reverse engineer the cooler{Enter}");
    expect(invoke).toHaveBeenCalledWith("set_atom_text", {
      atomId: "a1",
      text: "Reverse engineer the cooler",
    });
  });
});

describe("Snippet", () => {
  it("highlights FTS hits", () => {
    const { container } = render(<Snippet text="the [linux] control [center]" />);
    expect([...container.querySelectorAll("mark")].map((m) => m.textContent)).toEqual([
      "linux",
      "center",
    ]);
  });
});

describe("ProcessingBanner", () => {
  it("reassures that thoughts are saved when processing is blocked", () => {
    render(
      <ProcessingBanner
        status={{
          enabled: true,
          provider: "openrouter",
          blocked_reason: "Add your OpenRouter API key in Settings to process captures.",
          queued: 3,
          running: 0,
          failed: 0,
        }}
        onChanged={() => {}}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Your thoughts are saved.");
    expect(screen.getByRole("status")).toHaveTextContent("3 waiting");
  });
});
