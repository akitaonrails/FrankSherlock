import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import Titlebar from "../../components/Titlebar/Titlebar";

const baseProps = {
  onClose: vi.fn(),
  subtitle: null as string | null,
  sidebarCollapsed: false,
  onToggleSidebar: vi.fn(),
};

describe("Titlebar", () => {
  it("renders the app title", () => {
    render(<Titlebar {...baseProps} />);
    expect(screen.getByText("Frank Sherlock")).toBeInTheDocument();
  });

  it("renders minimize, maximize, close buttons", () => {
    render(<Titlebar {...baseProps} />);
    expect(screen.getByLabelText("Minimize")).toBeInTheDocument();
    expect(screen.getByLabelText("Maximize")).toBeInTheDocument();
    expect(screen.getByLabelText("Close")).toBeInTheDocument();
  });

  it("calls onClose when close button is clicked", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Titlebar {...baseProps} onClose={onClose} />);
    await user.click(screen.getByLabelText("Close"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("displays subtitle when provided", () => {
    render(<Titlebar {...baseProps} subtitle="My Folder" />);
    expect(screen.getByText("Frank Sherlock \u2014 My Folder")).toBeInTheDocument();
  });

  it("shows no subtitle separator when subtitle is null", () => {
    render(<Titlebar {...baseProps} subtitle={null} />);
    expect(screen.getByText("Frank Sherlock")).toBeInTheDocument();
    expect(screen.queryByText(/\u2014/)).not.toBeInTheDocument();
  });

  it("renders toggle button and calls onToggleSidebar", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    render(<Titlebar {...baseProps} onToggleSidebar={onToggle} />);
    const btn = screen.getByLabelText("Hide sidebar");
    expect(btn).toBeInTheDocument();
    await user.click(btn);
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("toggle button label changes when sidebar is collapsed", () => {
    render(<Titlebar {...baseProps} sidebarCollapsed={true} />);
    expect(screen.getByLabelText("Show sidebar")).toBeInTheDocument();
  });
});
