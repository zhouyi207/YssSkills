import { getIpcErrorDetails } from "@/app/services/ipc-error-presentation";
import { cn } from "@/lib/utils";
import type { IpcError } from "@/shared/types/ipc";

export function IpcErrorDetails({
  error,
  compact = false,
  className,
}: {
  error: IpcError;
  compact?: boolean;
  className?: string;
}) {
  const details = getIpcErrorDetails(error);
  const messageClassName = compact ? "text-xs" : "text-sm";

  return (
    <div className={cn("min-w-0 space-y-1 text-muted-foreground", className)}>
      <p className={messageClassName}>{error.message}</p>
      {details.length > 0 ? (
        <dl className={cn("space-y-0.5", compact ? "text-[0.65rem]" : "text-xs")}>
          {details.map(({ label, value }, index) => (
            <div key={`${label}-${index}`} className="flex min-w-0 flex-wrap gap-x-1">
              <dt className="font-medium text-foreground/80">{label}:</dt>
              <dd className="min-w-0 break-words">{value}</dd>
            </div>
          ))}
        </dl>
      ) : null}
      <p className="text-[0.65rem]">
        Error code: <code>{error.code}</code>
      </p>
    </div>
  );
}
