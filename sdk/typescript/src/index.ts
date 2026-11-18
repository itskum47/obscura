/**
 * Official TypeScript SDK for the Obscura Next Autonomous Browser Platform.
 * Communicates with the obscura-worker binary over framed JSON lines.
 */

import { spawn, ChildProcess } from 'child_process';
import * as readline from 'readline';
import * as fs from 'fs';
import * as path from 'path';
import { ActionResult, SessionConfig, WaitUntil, WorkerResponse } from './types';

export * from './types';

/**
 * Resolves the obscura-worker binary in priority order.
 */
export function resolveWorkerBinary(configured?: string): string {
  if (configured && fs.existsSync(configured)) {
    return configured;
  }

  const envPath = process.env.OBSCURA_WORKER_BIN;
  if (envPath && fs.existsSync(envPath)) {
    return envPath;
  }

  const candidates = [
    path.resolve(process.cwd(), 'target/debug/obscura-worker'),
    path.resolve(process.cwd(), 'target/release/obscura-worker'),
    path.resolve(process.cwd(), '../target/debug/obscura-worker'),
    path.resolve(process.cwd(), '../target/release/obscura-worker'),
    path.resolve(process.cwd(), '../../target/debug/obscura-worker'),
    path.resolve(process.cwd(), '../../target/release/obscura-worker'),
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return configured || 'obscura-worker';
}

interface PendingRequest {
  resolve: (value: WorkerResponse) => void;
  reject: (reason: Error) => void;
  timer?: NodeJS.Timeout;
}

export class ObscuraClient {
  private binaryPath: string;
  private defaultSessionId: string;
  private proc: ChildProcess | null = null;
  private rl: readline.Interface | null = null;
  private pendingResolvers: Map<string, PendingRequest> = new Map();
  private reqCounter = 0;

  constructor(binaryPath?: string, defaultSessionId?: string) {
    this.binaryPath = resolveWorkerBinary(binaryPath);
    this.defaultSessionId = defaultSessionId || `ts-session-${Date.now().toString(36)}`;
  }

  private nextId(): string {
    this.reqCounter++;
    return `ts-${this.reqCounter}`;
  }

  private sid(sessionId?: string): string {
    return sessionId !== undefined ? sessionId : this.defaultSessionId;
  }

  public async start(handshakeTimeoutMs = 10000): Promise<void> {
    this.proc = spawn(this.binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    if (!this.proc.stdout || !this.proc.stdin) {
      if (this.proc) {
        this.proc.kill('SIGKILL');
        this.proc = null;
      }
      throw new Error('Failed to acquire worker stdio handles');
    }

    this.rl = readline.createInterface({ input: this.proc.stdout });

    // Wait for the initial {"ready":true} handshake
    await new Promise<void>((resolve, reject) => {
      let settled = false;
      const cleanup = () => {
        settled = true;
        if (timer) clearTimeout(timer);
        if (this.rl) this.rl.removeListener('line', onInitialLine);
        if (this.proc) {
          this.proc.removeListener('error', onError);
          this.proc.removeListener('exit', onExit);
        }
      };

      const fail = (err: Error) => {
        if (settled) return;
        cleanup();
        if (this.proc && !this.proc.killed && this.proc.exitCode === null) {
          try {
            this.proc.kill('SIGKILL');
          } catch {
            // ignore
          }
        }
        this.proc = null;
        if (this.rl) {
          this.rl.close();
          this.rl = null;
        }
        reject(err);
      };

      const timer = setTimeout(() => {
        fail(new Error(`Worker handshake timed out after ${handshakeTimeoutMs}ms`));
      }, handshakeTimeoutMs);
      timer.unref();

      const onInitialLine = (line: string) => {
        try {
          const parsed = JSON.parse(line);
          if (parsed.ready) {
            cleanup();
            resolve();
          } else {
            fail(new Error(`Worker handshake failed: ${line}`));
          }
        } catch (e) {
          fail(new Error(`Failed to parse worker handshake: ${line}`));
        }
      };

      const onError = (err: Error) => fail(err);
      const onExit = (code: number | null, signal: string | null) =>
        fail(new Error(`Worker process exited prematurely during handshake (code: ${code}, signal: ${signal})`));

      this.rl!.once('line', onInitialLine);
      this.proc!.once('error', onError);
      this.proc!.once('exit', onExit);
    });

    // Rejection on unexpected child exit
    this.proc.once('exit', (code, signal) => {
      const err = new Error(`Worker process terminated unexpectedly (code: ${code}, signal: ${signal})`);
      for (const req of this.pendingResolvers.values()) {
        if (req.timer) clearTimeout(req.timer);
        req.reject(err);
      }
      this.pendingResolvers.clear();
    });

    // Handle subsequent response lines
    this.rl.on('line', (line: string) => {
      try {
        const resp = JSON.parse(line) as WorkerResponse;
        if (resp.id) {
          const entry = this.pendingResolvers.get(resp.id);
          if (entry) {
            this.pendingResolvers.delete(resp.id);
            if (entry.timer) clearTimeout(entry.timer);
            entry.resolve(resp);
          }
        }
      } catch {
        // Ignore malformed JSON output
      }
    });
  }

  public sendCommand<T = WorkerResponse>(cmd: Record<string, unknown>, timeoutMs = 30000): Promise<T> {
    return new Promise((resolve, reject) => {
      if (!this.proc || !this.proc.stdin || this.proc.exitCode !== null) {
        return reject(new Error('Worker process is not running'));
      }

      const id = (cmd.id as string) || this.nextId();
      cmd.id = id;
      if (!cmd.version) {
        cmd.version = '1.0';
      }

      let timer: NodeJS.Timeout | undefined;
      if (timeoutMs > 0) {
        timer = setTimeout(() => {
          this.pendingResolvers.delete(id);
          reject(new Error(`Command '${cmd.cmd}' (id: ${id}) timed out after ${timeoutMs}ms`));
        }, timeoutMs);
        timer.unref();
      }

      this.pendingResolvers.set(id, {
        resolve: resolve as unknown as (value: WorkerResponse) => void,
        reject,
        timer,
      });

      this.proc.stdin.write(JSON.stringify(cmd) + '\n', (err) => {
        if (err) {
          if (timer) clearTimeout(timer);
          this.pendingResolvers.delete(id);
          reject(err);
        }
      });
    });
  }

  public async ping(): Promise<WorkerResponse> {
    return this.sendCommand({ cmd: 'ping' });
  }

  public async health(): Promise<WorkerResponse> {
    return this.sendCommand({ cmd: 'health' });
  }

  public async createSession(sessionId?: string, config?: SessionConfig): Promise<WorkerResponse> {
    const sid = this.sid(sessionId);
    const cmd: Record<string, unknown> = {
      cmd: 'session_create',
      session_id: sid,
    };
    if (config) {
      cmd.config = {
        proxy_url: config.proxyUrl,
        stealth: config.stealth ?? true,
        user_agent: config.userAgent,
        allow_file_access: config.allowFileAccess ?? false,
        allow_private_network: config.allowPrivateNetwork ?? false,
        timeout_ms: config.timeoutMs ?? 30000,
      };
    }
    return this.sendCommand(cmd);
  }

  public async destroySession(sessionId?: string): Promise<WorkerResponse> {
    return this.sendCommand({
      cmd: 'session_destroy',
      session_id: this.sid(sessionId),
    });
  }

  public async sessionStatus(sessionId?: string): Promise<WorkerResponse> {
    return this.sendCommand({
      cmd: 'session_status',
      session_id: this.sid(sessionId),
    });
  }

  public async navigate(
    url: string,
    sessionId?: string,
    waitUntil: WaitUntil = 'load',
    timeoutMs?: number
  ): Promise<ActionResult> {
    const t0 = Date.now();
    const cmd: Record<string, unknown> = {
      cmd: 'navigate',
      session_id: this.sid(sessionId),
      url,
      waitUntil,
    };
    if (timeoutMs !== undefined) {
      cmd.timeout_ms = timeoutMs;
    }
    const resp = await this.sendCommand<WorkerResponse>(cmd);
    const elapsed = Date.now() - t0;
    return {
      action: 'navigate',
      success: resp.ok,
      payload: resp.result,
      elapsedMs: elapsed,
      error: resp.error ? (typeof resp.error === 'string' ? resp.error : resp.error.message) : undefined,
    };
  }

  public async evaluate(
    expression: string,
    sessionId?: string,
    awaitPromise = true,
    timeoutMs?: number
  ): Promise<ActionResult> {
    const t0 = Date.now();
    const cmd: Record<string, unknown> = {
      cmd: 'evaluate',
      session_id: this.sid(sessionId),
      expression,
      await_promise: awaitPromise,
    };
    if (timeoutMs !== undefined) {
      cmd.timeout_ms = timeoutMs;
    }
    const resp = await this.sendCommand<WorkerResponse>(cmd);
    const elapsed = Date.now() - t0;
    return {
      action: 'evaluate',
      success: resp.ok,
      payload: resp.result,
      elapsedMs: elapsed,
      error: resp.error ? (typeof resp.error === 'string' ? resp.error : resp.error.message) : undefined,
    };
  }

  public async snapshot(sessionId?: string): Promise<WorkerResponse> {
    return this.sendCommand({
      cmd: 'snapshot',
      session_id: this.sid(sessionId),
    });
  }

  public async click(selector: string, sessionId?: string): Promise<ActionResult> {
    const t0 = Date.now();
    const resp = await this.sendCommand<WorkerResponse>({
      cmd: 'click',
      session_id: this.sid(sessionId),
      selector,
    });
    const elapsed = Date.now() - t0;
    return {
      action: 'click',
      success: resp.ok,
      payload: resp.result,
      elapsedMs: elapsed,
      error: resp.error ? (typeof resp.error === 'string' ? resp.error : resp.error.message) : undefined,
    };
  }

  public async fill(selector: string, value: string, sessionId?: string): Promise<ActionResult> {
    const t0 = Date.now();
    const resp = await this.sendCommand<WorkerResponse>({
      cmd: 'fill',
      session_id: this.sid(sessionId),
      selector,
      value,
    });
    const elapsed = Date.now() - t0;
    return {
      action: 'fill',
      success: resp.ok,
      payload: resp.result,
      elapsedMs: elapsed,
      error: resp.error ? (typeof resp.error === 'string' ? resp.error : resp.error.message) : undefined,
    };
  }

  public async typeText(selector: string, text: string, sessionId?: string): Promise<ActionResult> {
    const t0 = Date.now();
    const resp = await this.sendCommand<WorkerResponse>({
      cmd: 'type',
      session_id: this.sid(sessionId),
      selector,
      text,
    });
    const elapsed = Date.now() - t0;
    return {
      action: 'type',
      success: resp.ok,
      payload: resp.result,
      elapsedMs: elapsed,
      error: resp.error ? (typeof resp.error === 'string' ? resp.error : resp.error.message) : undefined,
    };
  }

  public async screenshot(sessionId?: string): Promise<WorkerResponse> {
    return this.sendCommand({
      cmd: 'screenshot',
      session_id: this.sid(sessionId),
    });
  }

  public async title(sessionId?: string): Promise<string> {
    const resp = await this.sendCommand<{ ok: boolean; result?: string }>({
      cmd: 'title',
      session_id: this.sid(sessionId),
    });
    return resp.result || '';
  }

  public async dump(format: 'html' | 'text' = 'html', sessionId?: string): Promise<string> {
    const resp = await this.sendCommand<{ ok: boolean; result?: string }>({
      cmd: 'dump',
      session_id: this.sid(sessionId),
      format,
    });
    return resp.result || '';
  }

  public async close(): Promise<void> {
    if (!this.proc) {
      return;
    }

    try {
      await this.sendCommand({ cmd: 'shutdown' });
    } catch {
      // Process may already be exiting
    }

    if (this.rl) {
      this.rl.close();
      this.rl = null;
    }

    if (this.proc.stdin) {
      try {
        this.proc.stdin.end();
      } catch {
        // Ignore
      }
    }

    await new Promise<void>((resolve) => {
      if (!this.proc || this.proc.killed || this.proc.exitCode !== null) {
        return resolve();
      }

      const timer = setTimeout(() => {
        if (this.proc && !this.proc.killed) {
          this.proc.kill('SIGKILL');
        }
        resolve();
      }, 3000);
      timer.unref();

      this.proc.once('exit', () => {
        clearTimeout(timer);
        resolve();
      });
    });

    if (this.proc) {
      if (this.proc.stdout) {
        this.proc.stdout.destroy();
      }
      if (this.proc.stderr) {
        this.proc.stderr.destroy();
      }
    }

    this.proc = null;
    for (const entry of this.pendingResolvers.values()) {
      if (entry.timer) clearTimeout(entry.timer);
      entry.reject(new Error('Client closed'));
    }
    this.pendingResolvers.clear();
  }
}
