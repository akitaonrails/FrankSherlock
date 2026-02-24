import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import PdfViewer from "../../components/Content/PdfViewer";

const mockedInvoke = vi.mocked(invoke);

// Capture the onPassword callback passed to Document
let capturedOnPassword: ((cb: (pw: string | null) => void, reason: number) => void) | undefined;
let capturedOnLoadSuccess: ((result: { numPages: number }) => void) | undefined;

vi.mock("react-pdf", () => ({
  Document: ({ children, onPassword, onLoadSuccess }: {
    children: React.ReactNode;
    onPassword?: (cb: (pw: string | null) => void, reason: number) => void;
    onLoadSuccess?: (result: { numPages: number }) => void;
  }) => {
    capturedOnPassword = onPassword;
    capturedOnLoadSuccess = onLoadSuccess;
    return <div data-testid="pdf-document">{children}</div>;
  },
  Page: ({ pageNumber }: { pageNumber: number }) => (
    <div data-testid="pdf-page">Page {pageNumber}</div>
  ),
  pdfjs: { GlobalWorkerOptions: { workerSrc: "" } },
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: vi.fn(),
}));

describe("PdfViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedOnPassword = undefined;
    capturedOnLoadSuccess = undefined;
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [];
      return null;
    });
  });

  it("renders PDF document", () => {
    render(<PdfViewer filePath="/test/doc.pdf" />);
    expect(screen.getByTestId("pdf-document")).toBeInTheDocument();
  });

  it("shows password prompt when onPassword fires with no saved passwords", async () => {
    render(<PdfViewer filePath="/test/protected.pdf" />);

    await waitFor(() => {
      expect(capturedOnPassword).toBeDefined();
    });

    const NEED_PASSWORD = 1;
    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, NEED_PASSWORD);

    await waitFor(() => {
      expect(screen.getByText("This PDF is password-protected:")).toBeInTheDocument();
      expect(screen.getByPlaceholderText("Enter password")).toBeInTheDocument();
      expect(screen.getByText("Unlock")).toBeInTheDocument();
    });
  });

  it("submits password and saves on success", async () => {
    render(<PdfViewer filePath="/test/protected.pdf" />);

    await waitFor(() => {
      expect(capturedOnPassword).toBeDefined();
    });

    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, 1);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Enter password")).toBeInTheDocument();
    });

    await userEvent.type(screen.getByPlaceholderText("Enter password"), "mypass");
    await userEvent.click(screen.getByText("Unlock"));

    expect(mockCallback).toHaveBeenCalledWith("mypass");

    // Simulate successful load — triggers password save
    capturedOnLoadSuccess!({ numPages: 3 });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("add_pdf_password", {
        password: "mypass",
        label: "",
      });
    });
  });

  it("auto-tries saved passwords before prompting", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [
        { id: 1, password: "pw1", label: "", createdAt: 0 },
        { id: 2, password: "pw2", label: "", createdAt: 0 },
      ];
      return null;
    });

    render(<PdfViewer filePath="/test/protected.pdf" />);

    // Wait for saved passwords to load
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("list_pdf_passwords");
    });

    const mockCallback = vi.fn();

    // First call: should auto-try "pw1"
    capturedOnPassword!(mockCallback, 1);
    expect(mockCallback).toHaveBeenCalledWith("pw1");

    // Second call (pw1 was wrong): should auto-try "pw2"
    capturedOnPassword!(mockCallback, 2);
    expect(mockCallback).toHaveBeenCalledWith("pw2");

    // Third call (pw2 was wrong): should show prompt
    capturedOnPassword!(mockCallback, 2);
    await waitFor(() => {
      expect(screen.getByPlaceholderText("Enter password")).toBeInTheDocument();
    });
  });

  it("shows incorrect password message on manual retry", async () => {
    render(<PdfViewer filePath="/test/protected.pdf" />);

    await waitFor(() => {
      expect(capturedOnPassword).toBeDefined();
    });

    const mockCallback = vi.fn();

    // First prompt
    capturedOnPassword!(mockCallback, 1);
    await waitFor(() => {
      expect(screen.getByText("This PDF is password-protected:")).toBeInTheDocument();
    });

    // Submit wrong password
    await userEvent.type(screen.getByPlaceholderText("Enter password"), "wrong");
    await userEvent.click(screen.getByText("Unlock"));
    expect(mockCallback).toHaveBeenCalledWith("wrong");

    // onPassword called again with INCORRECT_PASSWORD
    capturedOnPassword!(mockCallback, 2);
    await waitFor(() => {
      expect(screen.getByText("Incorrect password. Try again:")).toBeInTheDocument();
    });
  });

  it("does not save saved password that works (already in DB)", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [
        { id: 1, password: "existing", label: "", createdAt: 0 },
      ];
      return null;
    });

    render(<PdfViewer filePath="/test/protected.pdf" />);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("list_pdf_passwords");
    });

    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, 1);
    expect(mockCallback).toHaveBeenCalledWith("existing");

    // Simulate success
    capturedOnLoadSuccess!({ numPages: 5 });

    // Should NOT call add_pdf_password since it was already saved
    await waitFor(() => {
      expect(mockedInvoke).not.toHaveBeenCalledWith("add_pdf_password", expect.anything());
    });
  });

  it("submits password on Enter key", async () => {
    render(<PdfViewer filePath="/test/protected.pdf" />);

    await waitFor(() => {
      expect(capturedOnPassword).toBeDefined();
    });

    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, 1);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Enter password")).toBeInTheDocument();
    });

    await userEvent.type(screen.getByPlaceholderText("Enter password"), "mypass{enter}");
    expect(mockCallback).toHaveBeenCalledWith("mypass");
  });

  it("triggers reclassify_pdf after manual password succeeds when fileId is provided", async () => {
    render(<PdfViewer filePath="/test/protected.pdf" fileId={42} />);

    await waitFor(() => {
      expect(capturedOnPassword).toBeDefined();
    });

    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, 1);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Enter password")).toBeInTheDocument();
    });

    await userEvent.type(screen.getByPlaceholderText("Enter password"), "mypass");
    await userEvent.click(screen.getByText("Unlock"));

    // Simulate successful load
    capturedOnLoadSuccess!({ numPages: 3 });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("reclassify_pdf", { fileId: 42 });
    });
  });

  it("triggers reclassify_pdf after saved password succeeds when fileId is provided", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [
        { id: 1, password: "saved_pw", label: "", createdAt: 0 },
      ];
      return null;
    });

    render(<PdfViewer filePath="/test/protected.pdf" fileId={99} />);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("list_pdf_passwords");
    });

    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, 1);
    expect(mockCallback).toHaveBeenCalledWith("saved_pw");

    // Simulate success (saved password worked)
    capturedOnLoadSuccess!({ numPages: 2 });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("reclassify_pdf", { fileId: 99 });
    });
  });

  it("does not trigger reclassify_pdf when no fileId is provided", async () => {
    render(<PdfViewer filePath="/test/protected.pdf" />);

    await waitFor(() => {
      expect(capturedOnPassword).toBeDefined();
    });

    const mockCallback = vi.fn();
    capturedOnPassword!(mockCallback, 1);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Enter password")).toBeInTheDocument();
    });

    await userEvent.type(screen.getByPlaceholderText("Enter password"), "mypass");
    await userEvent.click(screen.getByText("Unlock"));

    capturedOnLoadSuccess!({ numPages: 3 });

    // Should save password but NOT reclassify (no fileId)
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("add_pdf_password", {
        password: "mypass",
        label: "",
      });
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith("reclassify_pdf", expect.anything());
  });
});
