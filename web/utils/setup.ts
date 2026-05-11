import type { Dashboard, SetupIssue } from "../types";

export function blockingSetupIssues(dashboard: Dashboard | null): SetupIssue[] {
  return (dashboard?.setup_issues ?? []).filter((issue) => issue.blocking);
}

export function setupIssueSummary(issues: SetupIssue[]): string {
  return issues.map((issue) => issue.message).join(" / ");
}
