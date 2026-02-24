import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import PdfPasswordsView from "../../components/Content/PdfPasswordsView";
import { mockPdfPassword, mockProtectedPdf } from "../fixtures";

const mockedInvoke = vi.mocked(invoke);

const defaultProps = {
  onBack: vi.fn(),
  onNotice: vi.fn(),
  onError: vi.fn(),
};

describe("PdfPasswordsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [];
      if (cmd === "list_protected_pdfs") return [];
      return null;
    });
  });

  it("renders empty state", async () => {
    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("No passwords saved yet.")).toBeInTheDocument();
      expect(screen.getByText("No password-protected PDFs found.")).toBeInTheDocument();
    });
  });

  it("renders password list", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [mockPdfPassword];
      if (cmd === "list_protected_pdfs") return [];
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      // Password is masked by default
      expect(screen.getByText("electricity bill")).toBeInTheDocument();
      expect(screen.getByText("Show")).toBeInTheDocument();
    });
  });

  it("reveals and hides password", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [mockPdfPassword];
      if (cmd === "list_protected_pdfs") return [];
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Show")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Show"));
    expect(screen.getByText("secret123")).toBeInTheDocument();
    expect(screen.getByText("Hide")).toBeInTheDocument();

    await userEvent.click(screen.getByText("Hide"));
    expect(screen.queryByText("secret123")).not.toBeInTheDocument();
  });

  it("adds a password", async () => {
    let passwords: (typeof mockPdfPassword)[] = [];
    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "list_pdf_passwords") return passwords;
      if (cmd === "list_protected_pdfs") return [];
      if (cmd === "add_pdf_password") {
        const a = args as Record<string, unknown>;
        const pw = { ...mockPdfPassword, password: a?.password as string, label: a?.label as string };
        passwords = [pw];
        return pw;
      }
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("No passwords saved yet.")).toBeInTheDocument();
    });

    const pwInput = screen.getByPlaceholderText("Password");
    const labelInput = screen.getByPlaceholderText("Label (optional)");
    await userEvent.type(pwInput, "mypass");
    await userEvent.type(labelInput, "test label");
    await userEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(defaultProps.onNotice).toHaveBeenCalledWith("Password saved");
    });
  });

  it("deletes a password", async () => {
    let passwords = [mockPdfPassword];
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return passwords;
      if (cmd === "list_protected_pdfs") return [];
      if (cmd === "delete_pdf_password") {
        passwords = [];
        return null;
      }
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("electricity bill")).toBeInTheDocument();
    });

    await userEvent.click(screen.getByText("Delete"));
    await waitFor(() => {
      expect(screen.getByText("No passwords saved yet.")).toBeInTheDocument();
    });
  });

  it("renders protected PDFs list", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [];
      if (cmd === "list_protected_pdfs") return [mockProtectedPdf];
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("bill.pdf")).toBeInTheDocument();
    });
  });

  it("retry button is disabled when no passwords saved", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [];
      if (cmd === "list_protected_pdfs") return [mockProtectedPdf];
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      const retryBtn = screen.getByText("Retry All");
      expect(retryBtn).toBeDisabled();
    });
  });

  it("retry button is disabled when no protected PDFs", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [mockPdfPassword];
      if (cmd === "list_protected_pdfs") return [];
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      const retryBtn = screen.getByText("Retry All");
      expect(retryBtn).toBeDisabled();
    });
  });

  it("calls retry and shows result", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [mockPdfPassword];
      if (cmd === "list_protected_pdfs") return [mockProtectedPdf];
      if (cmd === "retry_protected_pdfs") return { totalAttempted: 1, unlocked: 1, stillProtected: 0 };
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Retry All")).not.toBeDisabled();
    });

    await userEvent.click(screen.getByText("Retry All"));
    await waitFor(() => {
      expect(defaultProps.onNotice).toHaveBeenCalledWith("Unlocked 1 of 1 PDF(s)");
    });
  });

  it("calls onBack when back button is clicked", async () => {
    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("Back")).toBeInTheDocument();
    });
    await userEvent.click(screen.getByText("Back"));
    expect(defaultProps.onBack).toHaveBeenCalled();
  });

  it("shows warning about plain text storage", async () => {
    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText(/stored in plain text/)).toBeInTheDocument();
    });
  });

  it("displays stats in toolbar", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_pdf_passwords") return [mockPdfPassword];
      if (cmd === "list_protected_pdfs") return [mockProtectedPdf, { ...mockProtectedPdf, id: 101, filename: "other.pdf" }];
      return null;
    });

    render(<PdfPasswordsView {...defaultProps} />);
    await waitFor(() => {
      expect(screen.getByText("1")).toBeInTheDocument(); // 1 saved password
      expect(screen.getByText("2")).toBeInTheDocument(); // 2 protected PDFs
    });
  });
});
