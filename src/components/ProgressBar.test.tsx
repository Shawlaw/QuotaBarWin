import { render, screen } from "@testing-library/react";
import { ProgressBar } from "./ProgressBar";

test("ProgressBar clamps percent into 0..100", () => {
  render(<ProgressBar percent={140} label="weekly remaining" />);

  expect(screen.getByRole("progressbar", { name: "weekly remaining" })).toHaveAttribute(
    "aria-valuenow",
    "100"
  );
});
