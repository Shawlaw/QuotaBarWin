import { render, screen } from "@testing-library/react";
import { ProgressBar } from "./ProgressBar";

test("ProgressBar clamps percent into 0..100", () => {
  render(<ProgressBar percent={140} label="weekly remaining" />);

  expect(screen.getByRole("progressbar", { name: "weekly remaining" })).toHaveAttribute(
    "aria-valuenow",
    "100"
  );
});

test("ProgressBar fades green opacity as remaining percent falls", () => {
  const { rerender } = render(<ProgressBar percent={100} label="quota remaining" />);
  const fill = screen.getByRole("progressbar", { name: "quota remaining" }).firstElementChild;

  expect(fill).toHaveStyle({ opacity: "1" });

  rerender(<ProgressBar percent={0} label="quota remaining" />);

  expect(fill).toHaveStyle({ opacity: "0.1" });
});

test("ProgressBar can use a separate opacity percent", () => {
  render(<ProgressBar percent={80} opacityPercent={20} label="quota used" />);
  const fill = screen.getByRole("progressbar", { name: "quota used" }).firstElementChild;

  expect(fill).toHaveStyle({ width: "80%" });
  expect(fill).toHaveStyle({ opacity: "0.28" });
});
