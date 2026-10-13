import type { Enrollment } from "../api/types";

export type Os = "linux" | "windows" | "macos";

const RELEASES = "https://github.com/firsttris/stellwerk/releases/download";

/**
 * The one command shown for a pairing code. The install script comes from the GitHub release
 * of the Zentrale's own version, not from the Zentrale: a device that does not trust the
 * Zentrale's certificate yet (a private CA) can still fetch it, and the CA's fingerprint lets
 * the agent trust that CA (installer/install.sh). Windows needs PowerShell; the dialog says so.
 */
export function pairCommand(os: Os, origin: string, enrollment: Enrollment): string {
  const base = `${RELEASES}/v${enrollment.version}`;
  const ca = enrollment.caSha256;
  if (os === "windows") {
    const script = `& ([scriptblock]::Create((irm ${base}/install.ps1)))`;
    return [script, origin, enrollment.code, ca && `-CaSha256 ${ca}`].filter(Boolean).join(" ");
  }
  const args = [origin, enrollment.code, ca && `--ca-sha256 ${ca}`].filter(Boolean).join(" ");
  return `curl -fsSL ${base}/install.sh | sh -s -- ${args}`;
}
