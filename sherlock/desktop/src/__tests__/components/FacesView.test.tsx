import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import FacesView from "../../components/Content/FacesView";
import type { PersonInfo, FaceInfo } from "../../types";

const mockedInvoke = vi.mocked(invoke);

const mockPerson: PersonInfo = {
  id: 1,
  name: "Person 1",
  faceCount: 5,
  cropPath: "/cache/face_crops/1.jpg",
  thumbnailPath: "/cache/thumbs/img.jpg",
};

const mockPerson2: PersonInfo = {
  id: 2,
  name: "Alice",
  faceCount: 3,
  cropPath: null,
  thumbnailPath: "/cache/thumbs/img2.jpg",
};

const mockFace1: FaceInfo = {
  id: 10,
  personId: 1,
  fileId: 100,
  relPath: "photos/face_a.jpg",
  filename: "face_a.jpg",
  confidence: 0.95,
  cropPath: "/cache/face_crops/10.jpg",
};

const mockFace2: FaceInfo = {
  id: 11,
  personId: 1,
  fileId: 101,
  relPath: "photos/face_b.jpg",
  filename: "face_b.jpg",
  confidence: 0.88,
  cropPath: "/cache/face_crops/11.jpg",
};

const defaultProps = {
  onBack: vi.fn(),
  onSelectPerson: vi.fn(),
  onPreviewFile: vi.fn(),
  onNotice: vi.fn(),
  onError: vi.fn(),
};

describe("FacesView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [];
      return null;
    });
  });

  it("renders empty state", async () => {
    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText(/No faces clustered yet/)).toBeInTheDocument();
    });
  });

  it("renders person cards", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson, mockPerson2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
      expect(screen.getByText("Alice")).toBeInTheDocument();
    });
  });

  it("shows stats in toolbar", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson, mockPerson2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("2")).toBeInTheDocument(); // 2 people
      expect(screen.getByText("8")).toBeInTheDocument(); // 8 faces total
    });
  });

  it("shows person detail view when card is clicked", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Click the card — enters detail view (not onSelectPerson)
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);

    await waitFor(() => {
      expect(screen.getByText("Back to People")).toBeInTheDocument();
      expect(screen.getByText("View Photos")).toBeInTheDocument();
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
      expect(screen.getByText("face_b.jpg")).toBeInTheDocument();
    });
    // onSelectPerson should NOT have been called
    expect(defaultProps.onSelectPerson).not.toHaveBeenCalled();
  });

  it("calls onSelectPerson via View Photos in detail view", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Enter detail view
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("View Photos")).toBeInTheDocument();
    });

    // Click View Photos
    await userEvent.click(screen.getByText("View Photos"));
    expect(defaultProps.onSelectPerson).toHaveBeenCalledWith(1, "Person 1");
  });

  it("calls onBack when back button is clicked", async () => {
    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Back")).toBeInTheDocument();
    });
    await userEvent.click(screen.getByText("Back"));
    expect(defaultProps.onBack).toHaveBeenCalled();
  });

  it("inline rename triggers renamePerson", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "rename_person") return null;
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Click the name label to start editing
    await userEvent.click(screen.getByText("Person 1"));

    // Type a new name
    const input = screen.getByDisplayValue("Person 1");
    await userEvent.clear(input);
    await userEvent.type(input, "Bob{Enter}");

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("rename_person", { personId: 1, newName: "Bob" });
      expect(defaultProps.onNotice).toHaveBeenCalledWith('Renamed to "Bob"');
    });
  });

  it("displays face count badges", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      const badge = document.querySelector(".faces-card-badge");
      expect(badge).not.toBeNull();
      expect(badge!.textContent!.trim()).toBe("5");
    });
  });

  it("calls unassign_face_from_person via context menu Remove from Person", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      if (cmd === "unassign_face_from_person") return null;
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Enter detail view
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Click the first face card to select it
    const firstCard = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(firstCard);

    // Right-click to open context menu
    fireEvent.contextMenu(firstCard);

    await waitFor(() => {
      expect(screen.getByTestId("detail-context-menu")).toBeInTheDocument();
      expect(screen.getByText("Remove from Person")).toBeInTheDocument();
    });

    // Click Remove from Person
    await userEvent.click(screen.getByText("Remove from Person"));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("unassign_face_from_person", { faceId: 10 });
    });
  });

  it("returns to person grid when Back to People clicked", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Enter detail view
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("Back to People")).toBeInTheDocument();
    });

    // Click Back to People
    await userEvent.click(screen.getByText("Back to People"));

    // Should be back on person grid
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
      expect(screen.queryByText("Back to People")).not.toBeInTheDocument();
    });
  });

  it("removes person from grid when last face unassigned", async () => {
    const singleFacePerson: PersonInfo = { ...mockPerson, faceCount: 1 };
    let personList = [singleFacePerson];

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [...personList];
      if (cmd === "list_faces_for_person") return [mockFace1];
      if (cmd === "unassign_face_from_person") {
        // Simulate backend: person gets deleted after last face removed
        personList = [];
        return null;
      }
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Enter detail view
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Select the face and use context menu to remove
    const card = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(card);
    fireEvent.contextMenu(card);
    await waitFor(() => {
      expect(screen.getByText("Remove from Person")).toBeInTheDocument();
    });
    await userEvent.click(screen.getByText("Remove from Person"));

    // Should return to person grid with empty state
    await waitFor(() => {
      expect(screen.getByText(/No faces clustered yet/)).toBeInTheDocument();
    });
  });

  it("shows context menu on right-click", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    const card = screen.getByText("Person 1").closest(".faces-card")!;
    fireEvent.contextMenu(card);

    await waitFor(() => {
      expect(screen.getByTestId("faces-context-menu")).toBeInTheDocument();
      expect(screen.getByText("Shuffle")).toBeInTheDocument();
    });
  });

  it("shuffle calls set_representative_face", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      if (cmd === "set_representative_face") return null;
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    const card = screen.getByText("Person 1").closest(".faces-card")!;
    fireEvent.contextMenu(card);

    await waitFor(() => {
      expect(screen.getByText("Shuffle")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Shuffle"));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith(
        "set_representative_face",
        expect.objectContaining({ personId: 1 }),
      );
    });
  });

  it("context menu closes on Escape", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    const card = screen.getByText("Person 1").closest(".faces-card")!;
    fireEvent.contextMenu(card);

    await waitFor(() => {
      expect(screen.getByTestId("faces-context-menu")).toBeInTheDocument();
    });

    await userEvent.keyboard("{Escape}");

    await waitFor(() => {
      expect(screen.queryByTestId("faces-context-menu")).not.toBeInTheDocument();
    });
  });

  it("re-cluster shows confirmation dialog", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Re-cluster")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Re-cluster"));

    expect(confirmSpy).toHaveBeenCalled();
    // recluster_faces should NOT have been called since confirm returned false
    expect(mockedInvoke).not.toHaveBeenCalledWith("recluster_faces");

    confirmSpy.mockRestore();
  });

  it("face selection via click", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Enter detail view
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Click a face card to select it
    const firstCard = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(firstCard);

    expect(firstCard.classList.contains("selected")).toBe(true);
  });

  it("context menu shows Move to submenu", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson, mockPerson2];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    // Enter detail view for Person 1
    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Select a face and right-click
    const firstCard = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(firstCard);
    fireEvent.contextMenu(firstCard);

    await waitFor(() => {
      expect(screen.getByTestId("detail-context-menu")).toBeInTheDocument();
      expect(screen.getByText("Move to")).toBeInTheDocument();
      // Should show the other person (Alice) but not the current person (Person 1)
      expect(screen.getByText("Alice (3)")).toBeInTheDocument();
    });
  });

  it("ctrl+click toggles multi-select", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    const card1 = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    const card2 = screen.getByText("face_b.jpg").closest(".faces-detail-card")!;

    // Click first card
    await userEvent.click(card1);
    expect(card1.classList.contains("selected")).toBe(true);
    expect(card2.classList.contains("selected")).toBe(false);

    // Ctrl+click second card
    fireEvent.click(card2, { ctrlKey: true });
    expect(card1.classList.contains("selected")).toBe(true);
    expect(card2.classList.contains("selected")).toBe(true);

    // Ctrl+click first card again to deselect
    fireEvent.click(card1, { ctrlKey: true });
    expect(card1.classList.contains("selected")).toBe(false);
    expect(card2.classList.contains("selected")).toBe(true);
  });

  it("Ctrl+A selects all faces in detail view", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Press Ctrl+A
    await userEvent.keyboard("{Control>}a{/Control}");

    const card1 = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    const card2 = screen.getByText("face_b.jpg").closest(".faces-detail-card")!;
    expect(card1.classList.contains("selected")).toBe(true);
    expect(card2.classList.contains("selected")).toBe(true);
    // Toolbar should show selection count
    expect(screen.getByText(/2 selected/)).toBeInTheDocument();
  });

  it("Delete key removes selected faces from person", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      if (cmd === "unassign_face_from_person") return null;
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Select first face and press Delete
    const card = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(card);
    await userEvent.keyboard("{Delete}");

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("unassign_face_from_person", { faceId: 10 });
    });
  });

  it("Space previews selected faces", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Select first face and press Space
    const card = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(card);
    await userEvent.keyboard(" ");

    expect(defaultProps.onPreviewFile).toHaveBeenCalledWith([100]);
  });

  it("Move to action calls reassign_faces_to_person", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_persons") return [mockPerson, mockPerson2];
      if (cmd === "list_faces_for_person") return [mockFace1, mockFace2];
      if (cmd === "reassign_faces_to_person") return null;
      return null;
    });

    render(<FacesView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Person 1")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Person 1").closest(".faces-card")!);
    await waitFor(() => {
      expect(screen.getByText("face_a.jpg")).toBeInTheDocument();
    });

    // Select first face, right-click, click "Alice (3)" in Move to submenu
    const card = screen.getByText("face_a.jpg").closest(".faces-detail-card")!;
    await userEvent.click(card);
    fireEvent.contextMenu(card);

    await waitFor(() => {
      expect(screen.getByText("Alice (3)")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Alice (3)"));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("reassign_faces_to_person", {
        faceIds: [10],
        targetPersonId: 2,
      });
      expect(defaultProps.onNotice).toHaveBeenCalledWith('Moved 1 face(s) to Alice');
    });
  });
});
