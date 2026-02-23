import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RootCard from "../../components/Sidebar/RootCard";
import { mockRoot as sampleRoot, mockRunningScan } from "../fixtures";

describe("RootCard", () => {
  it("renders root name and file count", () => {
    render(
      <RootCard root={sampleRoot} isSelected={false} scan={undefined} readOnly={false} onSelect={vi.fn()} onDelete={vi.fn()} />
    );
    expect(screen.getByText("photos")).toBeInTheDocument();
    expect(screen.getByText("42 files")).toBeInTheDocument();
  });

  it("applies selected class when selected", () => {
    const { container } = render(
      <RootCard root={sampleRoot} isSelected scan={undefined} readOnly={false} onSelect={vi.fn()} onDelete={vi.fn()} />
    );
    expect(container.querySelector(".root-card.selected")).not.toBeNull();
  });

  it("calls onSelect when clicked", async () => {
    const onSelect = vi.fn();
    render(
      <RootCard root={sampleRoot} isSelected={false} scan={undefined} readOnly={false} onSelect={onSelect} onDelete={vi.fn()} />
    );
    await userEvent.click(screen.getByText("photos"));
    expect(onSelect).toHaveBeenCalled();
  });

  it("calls onDelete when delete button clicked", async () => {
    const onDelete = vi.fn();
    render(
      <RootCard root={sampleRoot} isSelected={false} scan={undefined} readOnly={false} onSelect={vi.fn()} onDelete={onDelete} />
    );
    await userEvent.click(screen.getByLabelText("Remove photos"));
    expect(onDelete).toHaveBeenCalled();
  });

  it("hides delete button in readOnly mode", () => {
    render(
      <RootCard root={sampleRoot} isSelected={false} scan={undefined} readOnly onSelect={vi.fn()} onDelete={vi.fn()} />
    );
    expect(screen.queryByLabelText("Remove photos")).not.toBeInTheDocument();
  });

  it("shows scan progress when scan is active", () => {
    render(
      <RootCard root={sampleRoot} isSelected={false} scan={mockRunningScan} readOnly={false} onSelect={vi.fn()} onDelete={vi.fn()} />
    );
    expect(screen.getByText("50/100")).toBeInTheDocument();
  });
});
