import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const importPaths = vi.fn(async () => ({ status: "ok", data: "job-1" }));

vi.mock("@/lib/ipc", async (original) => ({
  ...(await original<typeof import("@/lib/ipc")>()),
  commands: {
    listFolders: async () => ({
      status: "ok",
      data: [{ name: "Science", path: "Science", bookCount: 0, totalCount: 0, children: [] }],
    }),
    importPaths,
  },
}));

const { ImportDialog } = await import("./ImportDialog");
const { useImport } = await import("../import");

function renderDialog() {
  const client = new QueryClient();
  return render(
    <QueryClientProvider client={client}>
      <ImportDialog />
    </QueryClientProvider>,
  );
}

describe("ImportDialog", () => {
  beforeEach(() => {
    importPaths.mockClear();
    useImport.setState({ pending: null, mode: "move" });
  });

  it("stays hidden until something is dropped", () => {
    renderDialog();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("imports into the chosen folder with the chosen mode", async () => {
    useImport.getState().ask(["/home/jane/Downloads/a.pdf", "/home/jane/Papers"], "");
    renderDialog();
    expect(await screen.findByRole("dialog", { name: "Import 2 items" })).toBeInTheDocument();
    fireEvent.change(await screen.findByRole("combobox"), { target: { value: "Science" } });
    fireEvent.click(screen.getByLabelText(/Copy them/));
    fireEvent.click(screen.getByRole("button", { name: /^Import$/ }));
    await vi.waitFor(() =>
      expect(importPaths).toHaveBeenCalledWith(
        ["/home/jane/Downloads/a.pdf", "/home/jane/Papers"],
        "Science",
        "copy",
      ),
    );
    expect(useImport.getState().pending).toBeNull();
  });
});
