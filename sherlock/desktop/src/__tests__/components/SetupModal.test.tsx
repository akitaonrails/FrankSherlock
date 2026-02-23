import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import SetupModal from "../../components/modals/SetupModal";
import type { SetupStatus } from "../../types";

const mockSetup: SetupStatus = {
  isReady: false,
  ollamaAvailable: true,
  requiredModels: ["qwen2.5vl:7b"],
  missingModels: ["qwen2.5vl:7b"],
  instructions: ["Install Ollama", "Pull the model"],
  download: { status: "idle", progressPct: 0, message: "Ready to download" },
  pythonAvailable: false,
  pythonVersion: null,
  suryaVenvOk: false,
};

describe("SetupModal", () => {
  it("renders setup heading and instructions", () => {
    render(<SetupModal setup={mockSetup} onRecheck={() => {}} onDownload={() => {}} />);
    expect(screen.getByText("First-Time Setup")).toBeInTheDocument();
    expect(screen.getByText("Install Ollama")).toBeInTheDocument();
    expect(screen.getByText("Pull the model")).toBeInTheDocument();
  });

  it("shows Ollama status", () => {
    render(<SetupModal setup={mockSetup} onRecheck={() => {}} onDownload={() => {}} />);
    expect(screen.getByText("Running")).toBeInTheDocument();
  });

  it("calls onRecheck when Recheck clicked", async () => {
    const user = userEvent.setup();
    const onRecheck = vi.fn();
    render(<SetupModal setup={mockSetup} onRecheck={onRecheck} onDownload={() => {}} />);
    await user.click(screen.getByText("Recheck"));
    expect(onRecheck).toHaveBeenCalledOnce();
  });

  it("calls onDownload when Download clicked", async () => {
    const user = userEvent.setup();
    const onDownload = vi.fn();
    render(<SetupModal setup={mockSetup} onRecheck={() => {}} onDownload={onDownload} />);
    await user.click(screen.getByText("Download model"));
    expect(onDownload).toHaveBeenCalledOnce();
  });

  it("disables download button when running", () => {
    const running = {
      ...mockSetup,
      download: { ...mockSetup.download, status: "running" as const, progressPct: 50 },
    };
    render(<SetupModal setup={running} onRecheck={() => {}} onDownload={() => {}} />);
    expect(screen.getByText("Downloading...")).toBeDisabled();
  });
});
