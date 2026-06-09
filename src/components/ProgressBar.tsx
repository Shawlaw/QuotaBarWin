import { clampPercent } from "../lib/format";

type ProgressBarProps = {
  percent: number | null;
  opacityPercent?: number | null;
  label: string;
};

export function ProgressBar({ percent, opacityPercent = percent, label }: ProgressBarProps) {
  const safePercent = percent === null ? 0 : clampPercent(percent);
  const safeOpacityPercent = opacityPercent === null ? 0 : clampPercent(opacityPercent);
  const opacity = Number((0.1 + safeOpacityPercent * 0.009).toFixed(3));

  return (
    <div className="progress-block">
      <div
        className="progress-track"
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(safePercent)}
      >
        <div
          className="progress-fill"
          style={{ opacity, width: `${safePercent}%` }}
        />
      </div>
    </div>
  );
}
