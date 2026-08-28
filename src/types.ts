
export type KeyEvent = {

  kind: "pass" | "block";

  key: string | null;

  gapMs: number | null;

  total: number;
};

export type Snapshot = {
  thresholdMs: number;
  blocked: number;

  seen: number;
  filtering: boolean;

  error: string | null;
};
