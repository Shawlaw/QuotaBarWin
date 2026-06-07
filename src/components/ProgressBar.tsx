import { clampPercent, formatPercent } from "../lib/format";

type ProgressBarProps = {
  percent: number | null;
  label: string;
};

export function ProgressBar({ percent, label }: ProgressBarProps) {
  const safePercent = percent === null ? 0 : clampPercent(percent);

  return (
    <div className="progress-block">
      <div className="progress-label">
        <span>{label}</span>
        <span>{formatPercent(percent)}</span>
      </div>
      <div
        className="progress-track"
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(safePercent)}
      >
        <div className="progress-fill" style={{ width: `${safePercent}%` }} />
      </div>
    </div>
  );
}
